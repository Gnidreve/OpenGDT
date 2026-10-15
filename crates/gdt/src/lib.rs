//! GDT (Gerätedatentransfer) – zeilenbasierter Austausch mit Praxisverwaltungssystemen.
//!
//! Zeilenformat: `LLL` (3 Ziffern, Gesamtlänge der Zeile inkl. CRLF) + `FFFF`
//! (4 Ziffern, Feldkennung) + Inhalt + `CR LF`.

mod charset;
mod patient;
mod record;

pub use charset::Charset;
pub use patient::{ExamResult, ImageRef, Patient, Sex, WriteOptions};
pub use record::{Field, Record};

/// Feldkennungen, die in diesem Projekt verwendet werden.
///
/// ACHTUNG: Nicht alle Werte sind gegen die offizielle QMS-Spezifikation
/// geprüft, siehe `CLAUDE.md` in diesem Ordner (Abschnitt „Ungeprüft“).
pub mod ids {
    pub const RECORD_TYPE: u16 = 8000;
    pub const RECORD_LENGTH: u16 = 8100;
    pub const RECEIVER_ID: u16 = 8315;
    pub const SENDER_ID: u16 = 8316;
    pub const CHARSET: u16 = 9206;
    pub const GDT_VERSION: u16 = 9218;
    pub const PATIENT_ID: u16 = 3000;
    pub const LAST_NAME: u16 = 3101;
    pub const FIRST_NAME: u16 = 3102;
    pub const BIRTH_DATE: u16 = 3103;
    pub const SEX: u16 = 3110;
    pub const EXAM_DATE: u16 = 6200;
    pub const EXAM_TIME: u16 = 6201;
    pub const REMARK: u16 = 6228;
    // UNGEPRÜFT: Dateiverweis-Felder
    pub const FILE_FORMAT: u16 = 6303;
    pub const FILE_DESCRIPTION: u16 = 6304;
    pub const FILE_REF: u16 = 6305;
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    /// Zeile zu kurz oder Kennung/Länge nicht numerisch.
    MalformedLine(usize),
    /// Längenangabe stimmt nicht mit der Zeile überein (nur im strikten Modus).
    LengthMismatch {
        line: usize,
        declared: usize,
        actual: usize,
    },
    /// Pflichtfeld fehlt.
    MissingField(u16),
    /// Zeichen nicht im Zeichensatz darstellbar / Bytes nicht dekodierbar.
    Encoding,
    /// Feldinhalt hat ein unerwartetes Format.
    BadValue(u16),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::MalformedLine(n) => write!(f, "GDT-Zeile {n} ist fehlerhaft"),
            Error::LengthMismatch {
                line,
                declared,
                actual,
            } => {
                write!(
                    f,
                    "GDT-Zeile {line}: Länge {declared} deklariert, {actual} tatsächlich"
                )
            }
            Error::MissingField(id) => write!(f, "GDT-Feld {id} fehlt"),
            Error::Encoding => write!(f, "Zeichensatzfehler"),
            Error::BadValue(id) => write!(f, "GDT-Feld {id} hat ein ungültiges Format"),
        }
    }
}

impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patient::WriteOptions;

    fn req() -> Vec<u8> {
        let mut v = Vec::new();
        for (id, val) in [
            (8000, "6301"),
            (9206, "3"),
            (3000, "4711"),
            (3101, "Müller"),
            (3102, "Jürgen"),
            (3103, "07031965"),
            (3110, "1"),
        ] {
            let enc = Charset::Latin1.encode(val).unwrap();
            v.extend(format!("{:03}{id:04}", 9 + enc.len()).bytes());
            v.extend(enc);
            v.extend(b"\r\n");
        }
        v
    }

    #[test]
    fn parses_patient() {
        let rec = Record::parse(&req(), false).unwrap();
        assert_eq!(rec.charset(), Charset::Latin1);
        let p = Patient::from_record(&rec).unwrap();
        assert_eq!(p.id, "4711");
        assert_eq!(p.last_name, "Müller");
        assert_eq!(p.first_name, "Jürgen");
        assert_eq!(p.birth_date.as_deref(), Some("19650307"));
        assert_eq!(p.sex, Sex::Male);
    }

    #[test]
    fn strict_accepts_correct_lengths_and_rejects_wrong() {
        assert!(Record::parse(&req(), true).is_ok());
        let mut bad = req();
        bad[2] = b'9';
        assert!(matches!(
            Record::parse(&bad, true),
            Err(Error::LengthMismatch { .. })
        ));
        assert!(Record::parse(&bad, false).is_ok());
    }

    #[test]
    fn lf_only_files_are_accepted_leniently() {
        let data = b"0138000"
            .iter()
            .chain(b"6301\n")
            .copied()
            .collect::<Vec<u8>>();
        let rec = Record::parse(&data, false).unwrap();
        assert_eq!(rec.raw(8000), Some(&b"6301"[..]));
    }

    #[test]
    fn missing_patient_id_is_error() {
        let rec = Record::parse(b"0138000" as &[u8], false).unwrap();
        assert_eq!(Patient::from_record(&rec), Err(Error::MissingField(3000)));
    }

    #[test]
    fn writer_roundtrip_and_8100_is_correct() {
        let patient = Patient {
            id: "4711".into(),
            last_name: "Müller".into(),
            first_name: "Jürgen".into(),
            birth_date: Some("19650307".into()),
            sex: Sex::Male,
        };
        let res = ExamResult {
            patient: patient.clone(),
            exam_date: "20261015".into(),
            exam_time: "103000".into(),
            images: vec![ImageRef {
                format: "JPG".into(),
                path: r"\\srv\us\4711\1.jpg".into(),
                description: "Bild 1".into(),
            }],
        };
        let opts = WriteOptions {
            record_type: "6310",
            receiver_id: "PVS",
            sender_id: "OPENGDT",
            charset: Charset::Latin1,
        };
        let bytes = res.to_record(&opts).unwrap().to_bytes();
        // Strikt parsebar, d. h. alle Zeilenlängen stimmen
        let rec = Record::parse(&bytes, true).unwrap();
        assert_eq!(rec.fields[0].id, 8000);
        assert_eq!(rec.fields[1].id, 8100);
        let declared: usize = std::str::from_utf8(rec.raw(8100).unwrap())
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(declared, bytes.len());
        assert_eq!(Patient::from_record(&rec).unwrap(), patient);
        assert_eq!(
            rec.get(6200, Charset::Latin1).unwrap().as_deref(),
            Some("15102026")
        );
    }

    #[test]
    fn charset_cp437_roundtrip_and_unencodable() {
        let b = Charset::Cp437.encode("Müßig").unwrap();
        assert_eq!(Charset::Cp437.decode(&b).unwrap(), "Müßig");
        assert_eq!(Charset::Latin1.encode("日本"), Err(Error::Encoding));
        assert_eq!(Charset::SevenBit.encode("ü"), Err(Error::Encoding));
    }
}
