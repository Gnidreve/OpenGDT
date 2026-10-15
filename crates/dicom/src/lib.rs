//! Minimaler DICOM-Stack ohne externe Abhängigkeiten.
//!
//! Umfang: Datensätze (Implicit/Explicit VR LE), Part-10-Dateien, Upper Layer
//! (Associations/PDUs) sowie die DIMSE-Dienste C-ECHO, C-FIND (nur SCP-Seite
//! produktiv genutzt) und C-STORE.

pub mod assoc;
pub mod dataset;
pub mod file;
pub mod pdu;
pub mod scp;
pub mod scu;
pub mod tag;
pub mod uid;

pub use dataset::{Dataset, Element, Value};
pub use tag::{t, Tag, Vr};

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    /// Daten enden unerwartet.
    Truncated,
    /// Strukturell ungültig.
    Invalid(&'static str),
    /// Protokollverletzung / unerwartetes PDU.
    Protocol(String),
    /// Gegenstelle hat die Association abgelehnt oder abgebrochen.
    Rejected(String),
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(e) => write!(f, "I/O: {e}"),
            Error::Truncated => write!(f, "DICOM-Daten abgeschnitten"),
            Error::Invalid(m) => write!(f, "ungültige DICOM-Daten: {m}"),
            Error::Protocol(m) => write!(f, "Protokollfehler: {m}"),
            Error::Rejected(m) => write!(f, "abgelehnt: {m}"),
        }
    }
}

impl std::error::Error for Error {}
