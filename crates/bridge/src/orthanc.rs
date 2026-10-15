//! Anbindung an einen **Orthanc**-Server über dessen REST-API (nur lesend).
//!
//! Die Bridge fragt `/changes` nach fertigen Untersuchungen (`StableStudy`: Orthanc meldet sie, wenn
//! `StableAge` Sekunden lang kein neues Bild kam), holt die Instanzen als DICOM-Dateien und schleust sie in
//! dasselbe Archiv/dieselbe Pipeline wie direkt empfangene Bilder. Orthanc bleibt der DICOM-Server.
//!
//! Verwendete Endpunkte (gegen Orthanc 1.12 geprüft): `GET /changes?since=&limit=`, `GET /changes?last`,
//! `GET /studies/{id}/instances`, `GET /instances/{id}/file`.

use crate::config::Config;
use crate::http::{self, Base};
use crate::pipeline::Pipeline;
use opengdt_dicom::file::read_part10;
use opengdt_dicom::scp::StoredObject;
use opengdt_json::Json;
use opengdt_store::Archive;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(120);
const PAGE: u64 = 100;
/// Nach so vielen Fehlversuchen an derselben Änderung wird sie übersprungen (kein dauerhaftes Blockieren).
const MAX_ATTEMPTS: u32 = 10;

/// Fehler beim Abruf. `permanent`: Wiederholen hilft nicht (z. B. Ressource existiert nicht mehr).
#[derive(Debug, PartialEq, Eq)]
pub struct Failure {
    pub permanent: bool,
    pub msg: String,
}

impl From<String> for Failure {
    fn from(msg: String) -> Self {
        Failure {
            permanent: false,
            msg,
        }
    }
}

impl From<&str> for Failure {
    fn from(msg: &str) -> Self {
        Failure {
            permanent: false,
            msg: msg.to_string(),
        }
    }
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.msg)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Change {
    pub seq: u64,
    pub kind: String,
    pub id: String,
}

#[derive(Debug)]
pub struct Changes {
    pub items: Vec<Change>,
    pub last: u64,
    pub done: bool,
}

pub struct Orthanc {
    base: Base,
    auth: Option<String>,
}

impl Orthanc {
    pub fn new(url: &str, user: &str, password: &str) -> Result<Orthanc, String> {
        let auth = (!user.is_empty()).then(|| http::basic_auth(user, password));
        Ok(Orthanc {
            base: http::parse_base(url)?,
            auth,
        })
    }

    fn get(&self, path: &str) -> Result<Vec<u8>, Failure> {
        let r = http::get(&self.base, path, self.auth.as_deref(), TIMEOUT)?;
        match r.status {
            200 => Ok(r.body),
            401 | 403 => Err(format!(
                "Orthanc verweigert den Zugriff (HTTP {}) – Benutzer/Passwort prüfen",
                r.status
            )
            .into()),
            404 => Err(Failure {
                permanent: true,
                msg: format!("Orthanc: {path} existiert nicht (mehr)"),
            }),
            s => Err(format!("Orthanc: HTTP {s} für {path}").into()),
        }
    }

    fn json(&self, path: &str) -> Result<Json, Failure> {
        let body = self.get(path)?;
        let text =
            String::from_utf8(body).map_err(|_| format!("{path}: Antwort ist kein UTF-8"))?;
        Ok(Json::parse(&text).map_err(|e| format!("{path}: {e}"))?)
    }

    pub fn changes(&self, since: u64, limit: u64) -> Result<Changes, Failure> {
        Ok(parse_changes(
            &self.json(&format!("/changes?since={since}&limit={limit}"))?,
        )?)
    }

    /// Laufende Nummer der letzten Änderung (Startpunkt, um alte Untersuchungen zu überspringen).
    pub fn last_seq(&self) -> Result<u64, Failure> {
        Ok(parse_changes(&self.json("/changes?last")?)?.last)
    }

    pub fn study_instances(&self, study: &str) -> Result<Vec<String>, Failure> {
        let j = self.json(&format!("/studies/{study}/instances"))?;
        let arr = j.as_arr().ok_or("Instanzliste ist kein Array")?;
        Ok(arr
            .iter()
            .map(|i| {
                i.get("ID")
                    .and_then(Json::as_str)
                    .map(String::from)
                    .ok_or_else(|| "Instanz ohne ID".to_string())
            })
            .collect::<Result<_, _>>()?)
    }

    pub fn instance_file(&self, instance: &str) -> Result<Vec<u8>, Failure> {
        self.get(&format!("/instances/{instance}/file"))
    }
}

/// Orthanc-IDs sind Hex mit Bindestrichen; alles andere kommt nicht in einen URL-Pfad.
fn safe_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 80 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

fn parse_changes(j: &Json) -> Result<Changes, String> {
    let items = j
        .get("Changes")
        .and_then(Json::as_arr)
        .ok_or("Antwort ohne 'Changes'")?
        .iter()
        .filter_map(|c| {
            Some(Change {
                seq: c.get("Seq")?.as_u64()?,
                kind: c.get("ChangeType")?.as_str()?.to_string(),
                id: c.get("ID")?.as_str()?.to_string(),
            })
        })
        .collect();
    Ok(Changes {
        items,
        last: j
            .get("Last")
            .and_then(Json::as_u64)
            .ok_or("Antwort ohne 'Last'")?,
        done: j.get("Done").and_then(Json::as_bool).unwrap_or(true),
    })
}

/// Holt fertige Untersuchungen aus Orthanc und übergibt sie der Pipeline.
pub struct Poller {
    orthanc: Orthanc,
    archive: Arc<Archive>,
    pipeline: Arc<Pipeline>,
    state_file: PathBuf,
    /// Höchste Änderungsnummer, bis zu der alles erledigt ist.
    pub since: u64,
    reachable: bool,
    /// Änderung, an der wir gerade scheitern, und wie oft in Folge.
    stuck: (u64, u32),
}

impl Poller {
    pub fn new(
        cfg: &Config,
        archive: Arc<Archive>,
        pipeline: Arc<Pipeline>,
    ) -> Result<Poller, String> {
        let orthanc = Orthanc::new(
            &cfg.orthanc_url,
            &cfg.orthanc_user,
            &cfg.orthanc_password()?,
        )?;
        let state_file = cfg.archive_dir.join(".orthanc-since");
        let saved = fs::read_to_string(&state_file)
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok());
        let mut p = Poller {
            orthanc,
            archive,
            pipeline,
            state_file,
            since: saved.unwrap_or(0),
            reachable: true,
            stuck: (0, 0),
        };
        if saved.is_none() {
            if cfg.orthanc_import_history {
                crate::info!(
                    "Orthanc: ohne gespeicherten Stand – gesamte Historie wird übernommen"
                );
            } else {
                // Beim ersten Start nur Neues bearbeiten: alte Untersuchungen lösen keine GDT-Rückmeldung aus.
                match p.orthanc.last_seq() {
                    Ok(last) => {
                        p.since = last;
                        p.save();
                        crate::info!("Orthanc: erster Start, setze Stand auf Änderung {last} (Historie wird übersprungen)");
                    }
                    Err(e) => crate::warn!("Orthanc nicht erreichbar ({e}) – Startpunkt wird beim nächsten Versuch bestimmt"),
                }
            }
        }
        Ok(p)
    }

    fn save(&self) {
        if let Err(e) = fs::write(&self.state_file, self.since.to_string()) {
            crate::warn!(
                "Orthanc-Stand nicht speicherbar ({}): {e}",
                self.state_file.display()
            );
        }
    }

    /// Ein Durchlauf. Gibt die Zahl übernommener Untersuchungen zurück.
    pub fn poll_once(&mut self) -> usize {
        let mut studies = 0;
        loop {
            let ch = match self.orthanc.changes(self.since, PAGE) {
                Ok(c) => c,
                Err(e) => {
                    if self.reachable {
                        crate::warn!("Orthanc nicht erreichbar: {e}");
                        self.reachable = false;
                    }
                    return studies;
                }
            };
            if !self.reachable {
                crate::info!("Orthanc wieder erreichbar");
                self.reachable = true;
            }
            if ch.last < self.since {
                // Orthanc wurde zurückgesetzt (Nummern beginnen von vorn).
                crate::warn!("Orthanc-Änderungsnummer ({}) liegt unter dem gespeicherten Stand ({}) – Stand zurückgesetzt", ch.last, self.since);
                self.since = ch.last;
                self.save();
                return studies;
            }
            for c in &ch.items {
                if c.kind == "StableStudy" {
                    match self.ingest_study(&c.id) {
                        Ok(n) => {
                            crate::info!(
                                "Orthanc: Untersuchung {} übernommen ({n} Instanzen)",
                                c.id
                            );
                            studies += 1;
                            self.stuck = (0, 0);
                        }
                        Err(e) => {
                            self.stuck = if self.stuck.0 == c.seq {
                                (c.seq, self.stuck.1 + 1)
                            } else {
                                (c.seq, 1)
                            };
                            if e.permanent || self.stuck.1 >= MAX_ATTEMPTS {
                                // Wiederholen bringt nichts: überspringen, damit spätere Untersuchungen nicht blockiert werden.
                                crate::error!(
                                    "Orthanc: Untersuchung {} wird übersprungen ({}. Versuch): {e}",
                                    c.id,
                                    self.stuck.1
                                );
                                self.stuck = (0, 0);
                            } else {
                                // Bei diesem Eintrag stehen bleiben; der nächste Durchlauf versucht ihn erneut.
                                crate::warn!("Orthanc: Untersuchung {} nicht übernommen (Versuch {}/{MAX_ATTEMPTS}): {e}", c.id, self.stuck.1);
                                self.save();
                                return studies;
                            }
                        }
                    }
                }
                self.since = c.seq;
            }
            if ch.done {
                self.since = self.since.max(ch.last);
                self.save();
                return studies;
            }
            self.save();
        }
    }

    fn ingest_study(&self, study: &str) -> Result<usize, Failure> {
        if !safe_id(study) {
            return Err(Failure {
                permanent: true,
                msg: format!("unzulässige ID '{study}'"),
            });
        }
        let ids = self.orthanc.study_instances(study)?;
        let mut n = 0;
        for id in ids {
            if !safe_id(&id) {
                crate::warn!("Orthanc: Instanz-ID '{id}' übersprungen");
                continue;
            }
            let bytes = self.orthanc.instance_file(&id)?;
            let p = match read_part10(&bytes) {
                Ok(p) => p,
                Err(e) => {
                    crate::warn!(
                        "Orthanc: Instanz {id} ist keine gültige DICOM-Datei ({e}) – übersprungen"
                    );
                    continue;
                }
            };
            let obj = StoredObject {
                sop_class: p.sop_class,
                sop_instance: p.sop_instance,
                transfer_syntax: p.transfer_syntax,
                data: p.data,
            };
            match self.archive.save(&obj) {
                Ok(saved) => {
                    self.pipeline.add(&saved);
                    n += 1;
                }
                Err(e) => return Err(format!("Archivierung: {e}").into()),
            }
        }
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_changes_response() {
        let j = Json::parse(r#"{"Changes":[{"ChangeType":"StableStudy","ID":"a-b","Seq":7,"Path":"/studies/a-b","ResourceType":"Study","Date":"x"},{"ChangeType":"NewInstance","ID":"i","Seq":8},{"bad":1}],"Done":false,"Last":99}"#).unwrap();
        let c = parse_changes(&j).unwrap();
        assert_eq!(
            c.items.len(),
            2,
            "Einträge ohne Pflichtfelder werden übersprungen"
        );
        assert_eq!(
            c.items[0],
            Change {
                seq: 7,
                kind: "StableStudy".into(),
                id: "a-b".into()
            }
        );
        assert_eq!((c.last, c.done), (99, false));
        assert!(parse_changes(&Json::parse("{}").unwrap()).is_err());
        assert!(parse_changes(&Json::parse(r#"{"Changes":[]}"#).unwrap()).is_err());
    }

    #[test]
    fn ids_are_path_safe() {
        assert!(safe_id("4a6d-b3c1-0000"));
        for bad in ["", "../x", "a/b", "a b", "a?b", "ä"] {
            assert!(!safe_id(bad), "{bad:?}");
        }
        assert!(!safe_id(&"a".repeat(81)));
    }
}
