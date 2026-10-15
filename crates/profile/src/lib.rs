//! Geräteprofile: maschinenlesbare Abschrift eines Herstellerdokuments (z. B. DICOM
//! Conformance Statement) mit Quellenangabe und Zeitstempel.
//!
//! Das Profil ist Datenquelle für Tests (und später evtl. für Voreinstellungen):
//! Wird die JSON-Datei neu aus einer neueren Fassung des Dokuments erzeugt,
//! prüfen die Tests automatisch gegen den neuen Stand.

pub mod json;
mod profile;

pub use json::Json;
pub use profile::{Key, Meta, Profile, SopClass, StorageContext, TransferSyntax};

/// Schema-Kennung, die `meta.schema` tragen muss.
pub const SCHEMA: &str = "opengdt-device-profile/1";
