//! Archiv für empfangene DICOM-Instanzen.
//!
//! Die Originaldaten bleiben die Quelle der Wahrheit: jede Instanz wird
//! unverändert als Part-10-Datei abgelegt, *bevor* irgendeine Konvertierung
//! läuft. Schlägt später etwas fehl, geht nichts verloren.

use opengdt_dicom::file::{is_explicit, write_part10};
use opengdt_dicom::scp::StoredObject;
use opengdt_dicom::{t, Dataset};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Wichtige Attribute einer Instanz, ohne Pixeldaten.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InstanceMeta {
    pub sop_class: String,
    pub sop_instance: String,
    pub transfer_syntax: String,
    pub patient_id: String,
    /// DICOM-PN, `Nachname^Vorname`
    pub patient_name: String,
    /// `YYYYMMDD` oder leer
    pub patient_birth_date: String,
    pub patient_sex: String,
    pub study_uid: String,
    pub accession: String,
    pub modality: String,
    pub study_date: String,
    pub study_time: String,
    pub instance_number: Option<u32>,
}

impl InstanceMeta {
    pub fn from_dataset(
        sop_class: &str,
        sop_instance: &str,
        ts: &str,
        ds: &Dataset,
    ) -> InstanceMeta {
        let s = |tag| ds.get_str(tag).unwrap_or_default();
        InstanceMeta {
            sop_class: sop_class.to_string(),
            sop_instance: sop_instance.to_string(),
            transfer_syntax: ts.to_string(),
            patient_id: s(t::PATIENT_ID),
            patient_name: s(t::PATIENT_NAME),
            patient_birth_date: s(t::PATIENT_BIRTH_DATE),
            patient_sex: s(t::PATIENT_SEX),
            study_uid: s(t::STUDY_INSTANCE_UID),
            accession: s(t::ACCESSION_NUMBER),
            modality: s(t::MODALITY),
            study_date: s(t::STUDY_DATE),
            study_time: s(t::STUDY_TIME),
            instance_number: ds
                .get_str(t::INSTANCE_NUMBER)
                .and_then(|v| v.trim().parse().ok()),
        }
    }
}

#[derive(Debug)]
pub struct Saved {
    pub path: PathBuf,
    pub meta: InstanceMeta,
}

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Dicom(opengdt_dicom::Error),
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}
impl From<opengdt_dicom::Error> for Error {
    fn from(e: opengdt_dicom::Error) -> Self {
        Error::Dicom(e)
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(e) => write!(f, "{e}"),
            Error::Dicom(e) => write!(f, "{e}"),
        }
    }
}
impl std::error::Error for Error {}

/// Dateisystemsicherer Name: verhindert Pfadtricks wie `../` aus DICOM-Feldern.
pub fn safe_component(s: &str) -> String {
    let c: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let c = c.trim_matches('.').to_string();
    if c.is_empty() {
        "unbekannt".into()
    } else {
        c.chars().take(64).collect()
    }
}

pub struct Archive {
    root: PathBuf,
}

impl Archive {
    pub fn open(root: impl Into<PathBuf>) -> io::Result<Archive> {
        let root = root.into();
        fs::create_dir_all(&root)?;
        Ok(Archive { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Legt `<root>/<patient>/<study>/<sop-instance>.dcm` an. Pixeldaten werden nicht im
    /// Speicher gehalten; Weiterverarbeitung liest die Datei erneut (`load`).
    pub fn save(&self, obj: &StoredObject) -> Result<Saved, Error> {
        let explicit = is_explicit(&obj.transfer_syntax).ok_or(opengdt_dicom::Error::Invalid(
            "Transfer Syntax nicht unterstützt",
        ))?;
        let dataset = Dataset::decode(&obj.data, explicit)?;
        let meta = InstanceMeta::from_dataset(
            &obj.sop_class,
            &obj.sop_instance,
            &obj.transfer_syntax,
            &dataset,
        );
        let dir = self
            .root
            .join(safe_component(&meta.patient_id))
            .join(safe_component(&meta.study_uid));
        fs::create_dir_all(&dir)?;
        let name = format!("{}.dcm", safe_component(&obj.sop_instance));
        let path = dir.join(&name);
        let tmp = dir.join(format!(".{name}.tmp"));
        fs::write(
            &tmp,
            write_part10(
                &obj.sop_class,
                &obj.sop_instance,
                &obj.transfer_syntax,
                &obj.data,
            ),
        )?;
        fs::rename(&tmp, &path)?;
        Ok(Saved { path, meta })
    }
}

/// Liest eine archivierte Instanz zurück: (Meta, Datensatz).
pub fn load(path: &Path) -> Result<(InstanceMeta, Dataset), Error> {
    let p = opengdt_dicom::file::read_part10(&fs::read(path)?)?;
    let explicit = is_explicit(&p.transfer_syntax).ok_or(opengdt_dicom::Error::Invalid(
        "Transfer Syntax nicht unterstützt",
    ))?;
    let ds = Dataset::decode(&p.data, explicit)?;
    let meta = InstanceMeta::from_dataset(&p.sop_class, &p.sop_instance, &p.transfer_syntax, &ds);
    Ok((meta, ds))
}

#[cfg(test)]
mod tests {
    use super::*;
    use opengdt_dicom::file::read_part10;
    use opengdt_dicom::uid;

    #[test]
    fn safe_component_blocks_traversal() {
        assert_eq!(
            safe_component("../../etc/passwd"),
            "_.._etc_passwd".trim_matches('.')
        );
        assert!(!safe_component("../x").contains('/'));
        assert_eq!(safe_component(".."), "unbekannt");
        assert_eq!(safe_component(""), "unbekannt");
        assert_eq!(safe_component("a b/c\\d"), "a_b_c_d");
    }

    #[test]
    fn save_writes_part10_and_extracts_meta() {
        let dir = std::env::temp_dir().join(format!("opengdt-test-store-{}", uid::generate()));
        let a = Archive::open(&dir).unwrap();
        let mut ds = Dataset::new();
        ds.set_str(t::PATIENT_ID, "../4711");
        ds.set_str(t::STUDY_INSTANCE_UID, "1.2.3");
        ds.set_str(t::MODALITY, "US");
        ds.set_str(t::INSTANCE_NUMBER, "7");
        let obj = StoredObject {
            sop_class: "1.2.840.10008.5.1.4.1.1.6.1".into(),
            sop_instance: "1.2.3.4".into(),
            transfer_syntax: uid::EXPLICIT_VR_LE.into(),
            data: ds.encode(true),
        };
        let s = a.save(&obj).unwrap();
        assert!(s.path.starts_with(&dir));
        assert_eq!(s.meta.patient_id, "../4711");
        assert_eq!(s.meta.instance_number, Some(7));
        let (m, d) = load(&s.path).unwrap();
        assert_eq!(m, s.meta);
        assert_eq!(d.get_str(t::MODALITY).as_deref(), Some("US"));
        let back = read_part10(&fs::read(&s.path).unwrap()).unwrap();
        assert_eq!(back.sop_instance, "1.2.3.4");
        assert_eq!(back.data, obj.data);
        let _ = fs::remove_dir_all(dir);
    }
}
