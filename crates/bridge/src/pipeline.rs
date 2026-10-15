//! Sammelt eingegangene Instanzen je Untersuchung, konvertiert sie nach einer
//! Ruhezeit (`settle_seconds`) und meldet sie per GDT ans PVS zurück.

use crate::config::Config;
use opengdt_convert::{convert_dataset, Options};
use opengdt_gdt::{Charset, ExamResult, ImageRef, Patient, Sex};
use opengdt_store::{load, safe_component, InstanceMeta, Saved};
use opengdt_worklist::time;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Marker im Archivverzeichnis einer Untersuchung: Rückmeldung ist erfolgt.
const REPORTED_MARKER: &str = ".reported";

struct Pending {
    last_update: Instant,
    retry_at: Option<Instant>,
    /// Fehler wurde bereits geloggt (verhindert Log-Flut bei Dauerzustand).
    warned: bool,
    files: Vec<PathBuf>,
}

/// Fehlschlag beim Abschließen einer Untersuchung.
struct Failure {
    msg: String,
    retry_secs: u64,
}

impl From<String> for Failure {
    fn from(msg: String) -> Self {
        Failure {
            msg,
            retry_secs: 30,
        }
    }
}

pub struct Pipeline {
    cfg: Arc<Config>,
    pending: Mutex<HashMap<String, Pending>>,
}

impl Pipeline {
    pub fn new(cfg: Arc<Config>) -> Pipeline {
        Pipeline {
            cfg,
            pending: Mutex::new(HashMap::new()),
        }
    }

    pub fn pending_studies(&self) -> usize {
        self.pending.lock().unwrap().len()
    }

    /// Schlüssel einer Untersuchung = ihr Archivverzeichnis.
    fn key_of(path: &Path) -> String {
        path.parent().unwrap_or(path).to_string_lossy().into_owned()
    }

    /// Nimmt nach einem Neustart Untersuchungen im Archiv wieder auf, die noch nicht
    /// zurückgemeldet wurden (kein `.reported`-Marker). Gibt ihre Anzahl zurück.
    pub fn recover(&self, archive_root: &Path) -> usize {
        let mut found = 0;
        let list = |p: &Path| -> Vec<PathBuf> {
            fs::read_dir(p)
                .map(|d| d.flatten().map(|e| e.path()).collect())
                .unwrap_or_default()
        };
        for patient in list(archive_root).into_iter().filter(|p| p.is_dir()) {
            for study in list(&patient).into_iter().filter(|p| p.is_dir()) {
                if study.join(REPORTED_MARKER).exists() {
                    continue;
                }
                let mut dcm: Vec<PathBuf> = list(&study)
                    .into_iter()
                    .filter(|p| p.extension().is_some_and(|e| e == "dcm"))
                    .collect();
                if dcm.is_empty() {
                    continue;
                }
                dcm.sort();
                let mut map = self.pending.lock().unwrap();
                map.insert(
                    study.to_string_lossy().into_owned(),
                    Pending {
                        last_update: Instant::now(),
                        retry_at: None,
                        warned: false,
                        files: dcm,
                    },
                );
                found += 1;
            }
        }
        found
    }

    pub fn add(&self, saved: &Saved) {
        let key = Self::key_of(&saved.path);
        let mut map = self.pending.lock().unwrap();
        let p = map.entry(key).or_insert_with(|| Pending {
            last_update: Instant::now(),
            retry_at: None,
            warned: false,
            files: vec![],
        });
        p.last_update = Instant::now();
        p.files.push(saved.path.clone());
    }

    /// Verarbeitet alle Untersuchungen, die lange genug ruhen (`force`: alle).
    /// Gibt die Zahl erfolgreich zurückgemeldeter Untersuchungen zurück.
    pub fn flush(&self, force: bool) -> usize {
        let settle = Duration::from_secs(self.cfg.settle_seconds);
        let now = Instant::now();
        let ready: Vec<(String, Vec<PathBuf>)> = {
            let map = self.pending.lock().unwrap();
            map.iter()
                .filter(|(_, p)| {
                    force
                        || (now.duration_since(p.last_update) >= settle
                            && p.retry_at.is_none_or(|t| now >= t))
                })
                .map(|(k, p)| (k.clone(), p.files.clone()))
                .collect()
        };
        let mut done = 0;
        for (key, files) in ready {
            match self.finish_study(&files) {
                Ok(outcome) => {
                    for dir in files
                        .iter()
                        .filter_map(|f| f.parent())
                        .collect::<std::collections::BTreeSet<_>>()
                    {
                        if let Err(e) = fs::write(dir.join(REPORTED_MARKER), b"") {
                            crate::warn!("Marker in {} nicht schreibbar: {e}", dir.display());
                        }
                    }
                    self.pending.lock().unwrap().remove(&key);
                    if matches!(outcome, Outcome::Done) {
                        done += 1;
                    }
                }
                Err(f) => {
                    if let Some(p) = self.pending.lock().unwrap().get_mut(&key) {
                        if !p.warned {
                            crate::error!("Untersuchung {key}: {} – wird wiederholt", f.msg);
                            p.warned = true;
                        }
                        p.retry_at = Some(Instant::now() + Duration::from_secs(f.retry_secs));
                    }
                }
            }
        }
        done
    }

    fn finish_study(&self, files: &[PathBuf]) -> Result<Outcome, Failure> {
        let mut first_meta: Option<InstanceMeta> = None;
        let mut exported: Vec<(u32, String, String, PathBuf)> = Vec::new(); // (Nr, Format, Beschreibung, rel. Pfad)
        let opts = Options {
            all_frames: self.cfg.all_frames,
        };

        for f in files {
            let (meta, ds) = match load(f) {
                Ok(x) => x,
                Err(e) => {
                    crate::warn!("{} nicht lesbar: {e}", f.display());
                    continue;
                }
            };
            let outputs = match convert_dataset(&meta.sop_class, &meta.transfer_syntax, &ds, opts) {
                Ok(o) => o,
                Err(e) => {
                    crate::warn!(
                        "{}: keine Konvertierung ({e}) – Original bleibt im Archiv",
                        f.display()
                    );
                    continue;
                }
            };
            let rel_dir = study_dir(&meta);
            let abs_dir = self.cfg.export_dir.join(&rel_dir);
            fs::create_dir_all(&abs_dir).map_err(|e| format!("{}: {e}", abs_dir.display()))?;
            let nr = meta.instance_number.unwrap_or(0);
            let short: String = safe_component(&meta.sop_instance)
                .chars()
                .rev()
                .take(8)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            for (k, out) in outputs.iter().enumerate() {
                let suffix = if k == 0 {
                    String::new()
                } else {
                    format!("_{k}")
                };
                let name = format!("{nr:03}_{short}{suffix}.{}", out.extension);
                let abs = abs_dir.join(&name);
                write_atomic(&abs, &out.bytes).map_err(|e| format!("{}: {e}", abs.display()))?;
                let desc = format!(
                    "Ultraschall {}",
                    match out.kind {
                        opengdt_convert::Kind::Video => "Video",
                        opengdt_convert::Kind::Document => "Dokument",
                        opengdt_convert::Kind::Still => "Bild",
                    }
                );
                exported.push((nr, out.gdt_format.to_string(), desc, rel_dir.join(&name)));
            }
            first_meta.get_or_insert(meta);
        }

        let Some(meta) = first_meta else {
            crate::warn!("Untersuchung ohne exportierbare Bilder – keine GDT-Rückmeldung");
            return Ok(Outcome::Nothing);
        };
        exported.sort_by(|a, b| (a.0, &a.3).cmp(&(b.0, &b.3)));

        let cs = self.cfg.gdt_charset;
        let lossy = |s: &str| lossy(cs, s);
        let (last, first) = split_name(&meta.patient_name);
        let patient = Patient {
            id: lossy(&meta.patient_id),
            last_name: lossy(&last),
            first_name: lossy(&first),
            birth_date: (meta.patient_birth_date.len() == 8)
                .then(|| meta.patient_birth_date.clone()),
            sex: match meta.patient_sex.as_str() {
                "M" => Sex::Male,
                "F" => Sex::Female,
                _ => Sex::Unknown,
            },
        };
        let (today, now_t) = time::date_time(time::now_unix(), self.cfg.utc_offset_minutes);
        let exam_date = if meta.study_date.len() == 8 {
            meta.study_date.clone()
        } else {
            today
        };
        let mut exam_time: String = meta.study_time.chars().take(6).collect();
        if exam_time.len() < 6 {
            exam_time = if exam_time.is_empty() {
                now_t
            } else {
                format!("{exam_time:0<6}")
            };
        }
        let images = exported
            .iter()
            .enumerate()
            .map(|(i, (_, fmt, desc, rel))| ImageRef {
                format: fmt.clone(),
                path: lossy(&self.pvs_path(rel)),
                description: format!("{desc} {}", i + 1),
            })
            .collect();
        let result = ExamResult {
            patient,
            exam_date,
            exam_time,
            images,
        };
        let record = result
            .to_record(&opengdt_gdt::WriteOptions {
                record_type: &self.cfg.gdt_result_type,
                receiver_id: &self.cfg.gdt_receiver_id,
                sender_id: &self.cfg.gdt_sender_id,
                charset: cs,
            })
            .map_err(|e| e.to_string())?;

        fs::create_dir_all(&self.cfg.gdt_export_dir).map_err(|e| e.to_string())?;
        let target = if self.cfg.gdt_result_filename.is_empty() {
            let id = if meta.accession.is_empty() {
                safe_component(&meta.sop_instance)
            } else {
                safe_component(&meta.accession)
            };
            self.cfg.gdt_export_dir.join(format!("{id}.gdt"))
        } else {
            self.cfg.gdt_export_dir.join(&self.cfg.gdt_result_filename)
        };
        if target.exists() {
            return Err(Failure {
                msg: format!(
                    "{} existiert noch (PVS hat sie nicht abgeholt)",
                    target.display()
                ),
                retry_secs: 2,
            });
        }
        write_atomic(&target, &record.to_bytes())
            .map_err(|e| format!("{}: {e}", target.display()))?;
        crate::info!(
            "GDT-Rückmeldung {} ({} Dateien, Patient {})",
            target.display(),
            result.images.len(),
            result.patient.id
        );
        Ok(Outcome::Done)
    }

    /// Pfad, unter dem das PVS die Datei sieht.
    fn pvs_path(&self, rel: &Path) -> String {
        let comps: Vec<String> = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect();
        let prefix = if self.cfg.pvs_export_prefix.is_empty() {
            std::path::absolute(&self.cfg.export_dir)
                .unwrap_or_else(|_| self.cfg.export_dir.clone())
                .to_string_lossy()
                .into_owned()
        } else {
            self.cfg.pvs_export_prefix.clone()
        };
        let sep = if prefix.contains('\\') { '\\' } else { '/' };
        let mut out = prefix.trim_end_matches(['/', '\\']).to_string();
        for c in comps {
            out.push(sep);
            out.push_str(&c);
        }
        out
    }
}

enum Outcome {
    Done,
    Nothing,
}

fn study_dir(meta: &InstanceMeta) -> PathBuf {
    let key = if !meta.accession.is_empty() {
        safe_component(&meta.accession)
    } else {
        let s = safe_component(&meta.study_uid);
        s.chars()
            .rev()
            .take(12)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    };
    let date = if meta.study_date.len() == 8 {
        meta.study_date.clone()
    } else {
        "00000000".into()
    };
    PathBuf::from(safe_component(&meta.patient_id)).join(format!("{date}_{key}"))
}

fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)
}

/// `Nachname^Vorname^...` → (Nachname, Vorname)
pub fn split_name(pn: &str) -> (String, String) {
    // Mehrere Zeichengruppen (`=`) – nur die erste (alphabetische) ist relevant.
    let first_group = pn.split('=').next().unwrap_or("");
    let mut parts = first_group.split('^');
    (
        parts.next().unwrap_or("").trim().to_string(),
        parts.next().unwrap_or("").trim().to_string(),
    )
}

/// Ersetzt Zeichen, die der GDT-Zeichensatz nicht kann, durch `?`.
fn lossy(cs: Charset, s: &str) -> String {
    s.chars()
        .map(|c| {
            if cs.encode(c.encode_utf8(&mut [0; 4])).is_ok() {
                c
            } else {
                '?'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_splitting() {
        assert_eq!(
            split_name("Müller^Jürgen^^^Dr"),
            ("Müller".into(), "Jürgen".into())
        );
        assert_eq!(split_name("Solo"), ("Solo".into(), "".into()));
        assert_eq!(split_name("A^B=ideo^graph"), ("A".into(), "B".into()));
        assert_eq!(split_name(""), ("".into(), "".into()));
    }

    #[test]
    fn lossy_replaces_unencodable() {
        assert_eq!(lossy(Charset::Latin1, "Müller 日"), "Müller ?");
        assert_eq!(lossy(Charset::SevenBit, "Müller"), "M?ller");
    }

    #[test]
    fn pvs_path_style_follows_prefix() {
        let cfg = Config {
            pvs_export_prefix: r"\\SRV\us\".into(),
            ..Config::default()
        };
        let p = Pipeline::new(Arc::new(cfg));
        assert_eq!(
            p.pvs_path(Path::new("4711/20261015_A1/001_x.jpg")),
            r"\\SRV\us\4711\20261015_A1\001_x.jpg"
        );
        let cfg = Config {
            pvs_export_prefix: "/mnt/us".into(),
            ..Config::default()
        };
        let p = Pipeline::new(Arc::new(cfg));
        assert_eq!(p.pvs_path(Path::new("a/b.jpg")), "/mnt/us/a/b.jpg");
    }
}
