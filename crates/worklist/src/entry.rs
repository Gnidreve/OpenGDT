//! GDT-Patient → DICOM-Worklist-Eintrag.

use crate::time;
use opengdt_dicom::{t, uid, Dataset};
use opengdt_gdt::{Patient, Sex};

pub struct EntryOptions {
    pub modality: String,
    /// AE-Titel des Ultraschallgeräts (Scheduled Station AE Title).
    pub station_ae: String,
    pub station_name: String,
    pub procedure_description: String,
    /// Versatz zur UTC in Minuten für Start-Datum/-Zeit des Eintrags.
    pub utc_offset_minutes: i32,
}

impl Default for EntryOptions {
    fn default() -> Self {
        EntryOptions {
            modality: "US".into(),
            station_ae: String::new(),
            station_name: String::new(),
            procedure_description: "Sonographie".into(),
            utc_offset_minutes: 0,
        }
    }
}

/// Latin-1 (`ISO_IR 100`); nicht darstellbare Zeichen → `?`.
fn latin1(s: &str) -> Vec<u8> {
    s.chars()
        .map(|c| u8::try_from(c as u32).unwrap_or(b'?'))
        .collect()
}

/// Entfernt DICOM-Trenner aus einer Namenskomponente.
fn component(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(c, '^' | '\\' | '\r' | '\n'))
        .collect::<String>()
        .trim()
        .to_string()
}

/// Seriennummer innerhalb des Prozesses für eindeutige Accession Numbers.
fn sequence() -> u32 {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    N.fetch_add(1, Ordering::Relaxed) % 1_000
}

pub fn build_entry(p: &Patient, o: &EntryOptions) -> Dataset {
    let (date, time_) = time::date_time(time::now_unix(), o.utc_offset_minutes);
    // VR SH erlaubt höchstens 16 Zeichen: `G` + JJMMTThhmmss + 3-stellige Folgenummer.
    let accession = format!("G{}{time_}{:03}", &date[2..], sequence());
    debug_assert!(accession.len() <= 16);

    let mut ds = Dataset::new();
    ds.set_str(t::SPECIFIC_CHARACTER_SET, "ISO_IR 100");
    let name = format!("{}^{}", component(&p.last_name), component(&p.first_name));
    ds.set_str_vr(
        t::PATIENT_NAME,
        opengdt_dicom::Vr::PN,
        latin1(name.trim_end_matches('^')),
    );
    ds.set_str_vr(
        t::PATIENT_ID,
        opengdt_dicom::Vr::LO,
        latin1(&component(&p.id)),
    );
    ds.set_str(t::PATIENT_BIRTH_DATE, p.birth_date.as_deref().unwrap_or(""));
    ds.set_str(
        t::PATIENT_SEX,
        match p.sex {
            Sex::Male => "M",
            Sex::Female => "F",
            Sex::Unknown => "",
        },
    );
    ds.set_str(t::STUDY_INSTANCE_UID, &uid::generate());
    ds.set_str(t::ACCESSION_NUMBER, &accession);
    ds.set_str(t::REFERRING_PHYSICIAN, "");
    ds.set_str(t::REQUESTING_PHYSICIAN, "");
    ds.set_str(t::REQUESTED_PROCEDURE_DESCRIPTION, &o.procedure_description);
    ds.set_str(t::REQUESTED_PROCEDURE_ID, &accession);
    ds.set_str(t::STUDY_ID, &accession);
    ds.set_str(t::REQUESTED_PROCEDURE_PRIORITY, "");

    let mut sps = Dataset::new();
    sps.set_str(t::MODALITY, &o.modality);
    sps.set_str(t::SPS_STATION_AE, &o.station_ae);
    sps.set_str(t::SPS_STATION_NAME, &o.station_name);
    sps.set_str(t::SPS_START_DATE, &date);
    sps.set_str(t::SPS_START_TIME, &time_);
    sps.set_str(t::SPS_PERFORMING_PHYSICIAN, "");
    sps.set_str(t::SPS_DESCRIPTION, &o.procedure_description);
    sps.set_str(t::SPS_ID, &accession);
    sps.set_str(t::SPS_LOCATION, "");
    sps.set_str(t::SPS_STATUS, "SCHEDULED");
    ds.set_seq(t::SPS_SEQUENCE, vec![sps]);
    ds
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_patient_fields() {
        let p = Patient {
            id: "4711".into(),
            last_name: "Müller".into(),
            first_name: "Jürgen".into(),
            birth_date: Some("19650307".into()),
            sex: Sex::Female,
        };
        let e = build_entry(
            &p,
            &EntryOptions {
                station_ae: "MINDRAY".into(),
                ..Default::default()
            },
        );
        // Latin-1 gespeichert, beim Lesen zurück nach Unicode
        assert_eq!(
            e.get_bytes(t::PATIENT_NAME).unwrap(),
            b"M\xFCller^J\xFCrgen"
        );
        assert_eq!(e.get_str(t::PATIENT_NAME).as_deref(), Some("Müller^Jürgen"));
        assert_eq!(e.get_str(t::PATIENT_SEX).as_deref(), Some("F"));
        let sps = &e.get_seq(t::SPS_SEQUENCE).unwrap()[0];
        assert_eq!(sps.get_str(t::MODALITY).as_deref(), Some("US"));
        assert_eq!(sps.get_str(t::SPS_STATION_AE).as_deref(), Some("MINDRAY"));
    }

    #[test]
    fn separators_and_exotic_chars_are_sanitized() {
        let p = Patient {
            id: "1".into(),
            last_name: "A^B".into(),
            first_name: "日本".into(),
            birth_date: None,
            sex: Sex::Unknown,
        };
        let e = build_entry(&p, &EntryOptions::default());
        assert_eq!(e.get_str(t::PATIENT_NAME).as_deref(), Some("AB^??"));
    }

    #[test]
    fn short_identifiers_respect_sh_limit() {
        let p = Patient {
            id: "1".into(),
            last_name: "A".into(),
            first_name: "".into(),
            birth_date: None,
            sex: Sex::Unknown,
        };
        let e = build_entry(&p, &EntryOptions::default());
        for tag in [t::ACCESSION_NUMBER, t::REQUESTED_PROCEDURE_ID, t::STUDY_ID] {
            assert!(e.get_str(tag).unwrap().len() <= 16, "{tag:?}");
        }
        let sps = &e.get_seq(t::SPS_SEQUENCE).unwrap()[0];
        assert!(sps.get_str(t::SPS_ID).unwrap().len() <= 16);
    }

    #[test]
    fn accession_numbers_are_unique() {
        let p = Patient {
            id: "1".into(),
            last_name: "A".into(),
            first_name: "".into(),
            birth_date: None,
            sex: Sex::Unknown,
        };
        let a = build_entry(&p, &EntryOptions::default());
        let b = build_entry(&p, &EntryOptions::default());
        assert_ne!(
            a.get_str(t::ACCESSION_NUMBER),
            b.get_str(t::ACCESSION_NUMBER)
        );
    }
}
