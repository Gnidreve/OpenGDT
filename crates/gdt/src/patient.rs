use crate::record::Builder;
use crate::{ids, Charset, Error, Record};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sex {
    Male,
    Female,
    Unknown,
}

/// Patientenstammdaten aus einer GDT-Untersuchungsanforderung.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Patient {
    pub id: String,
    pub last_name: String,
    pub first_name: String,
    /// ISO-Format `YYYYMMDD` (GDT liefert `TTMMJJJJ`).
    pub birth_date: Option<String>,
    pub sex: Sex,
}

impl Patient {
    pub fn from_record(rec: &Record) -> Result<Patient, Error> {
        let cs = rec.charset();
        let id = rec.require(ids::PATIENT_ID, cs)?;
        let last_name = rec.require(ids::LAST_NAME, cs)?;
        let first_name = rec.get(ids::FIRST_NAME, cs)?.unwrap_or_default();
        let birth_date = match rec.get(ids::BIRTH_DATE, cs)? {
            Some(d) => Some(gdt_date_to_iso(&d).ok_or(Error::BadValue(ids::BIRTH_DATE))?),
            None => None,
        };
        let sex = match rec.get(ids::SEX, cs)?.as_deref().map(str::trim) {
            Some("1") => Sex::Male,
            Some("2") => Sex::Female,
            _ => Sex::Unknown,
        };
        Ok(Patient {
            id,
            last_name,
            first_name,
            birth_date,
            sex,
        })
    }
}

/// `TTMMJJJJ` → `YYYYMMDD`.
pub fn gdt_date_to_iso(d: &str) -> Option<String> {
    let d = d.trim();
    if d.len() != 8 || !d.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(format!("{}{}{}", &d[4..8], &d[2..4], &d[0..2]))
}

/// `YYYYMMDD` → `TTMMJJJJ`.
pub fn iso_date_to_gdt(d: &str) -> Option<String> {
    if d.len() != 8 || !d.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(format!("{}{}{}", &d[6..8], &d[4..6], &d[0..4]))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageRef {
    /// z. B. `JPG`, `MP4`, `PNG`
    pub format: String,
    pub path: String,
    pub description: String,
}

/// Rückmeldung an das PVS: Untersuchung abgeschlossen, Bilder liegen unter den Pfaden.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExamResult {
    pub patient: Patient,
    /// `YYYYMMDD`
    pub exam_date: String,
    /// `HHMMSS`
    pub exam_time: String,
    pub images: Vec<ImageRef>,
}

pub struct WriteOptions<'a> {
    /// Satzart der Antwort (konfigurierbar, siehe CLAUDE.md „Ungeprüft“).
    pub record_type: &'a str,
    /// Kennung des Empfängers (PVS) und Senders (Bridge).
    pub receiver_id: &'a str,
    pub sender_id: &'a str,
    pub charset: Charset,
}

impl ExamResult {
    pub fn to_record(&self, o: &WriteOptions) -> Result<Record, Error> {
        let mut b = Builder::new(o.charset);
        b.add(ids::RECORD_TYPE, o.record_type)?;
        // 8100 wird beim Serialisieren eingefügt.
        b.add(ids::GDT_VERSION, "02.10")?;
        b.add(ids::RECEIVER_ID, o.receiver_id)?;
        b.add(ids::SENDER_ID, o.sender_id)?;
        b.add(ids::CHARSET, o.charset.field_value())?;
        b.add(ids::PATIENT_ID, &self.patient.id)?;
        b.add(ids::LAST_NAME, &self.patient.last_name)?;
        b.add(ids::FIRST_NAME, &self.patient.first_name)?;
        if let Some(d) = self.patient.birth_date.as_deref().and_then(iso_date_to_gdt) {
            b.add(ids::BIRTH_DATE, &d)?;
        }
        match self.patient.sex {
            Sex::Male => b.add(ids::SEX, "1")?,
            Sex::Female => b.add(ids::SEX, "2")?,
            Sex::Unknown => {}
        }
        let d = iso_date_to_gdt(&self.exam_date).ok_or(Error::BadValue(ids::EXAM_DATE))?;
        b.add(ids::EXAM_DATE, &d)?;
        b.add(ids::EXAM_TIME, &self.exam_time)?;
        for img in &self.images {
            b.add(ids::FILE_FORMAT, &img.format)?;
            b.add(ids::FILE_DESCRIPTION, &img.description)?;
            b.add(ids::FILE_REF, &img.path)?;
        }
        Ok(b.finish())
    }
}
