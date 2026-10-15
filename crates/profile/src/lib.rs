//! Geräteprofile: maschinenlesbare Abschrift eines Herstellerdokuments (z. B. DICOM
//! Conformance Statement) mit Quellenangabe und Zeitstempel.
//!
//! Das Profil ist Datenquelle für Tests (und später evtl. für Voreinstellungen):
//! Wird die JSON-Datei neu aus einer neueren Fassung des Dokuments erzeugt,
//! prüfen die Tests automatisch gegen den neuen Stand.

mod profile;

/// Der JSON-Parser lebt im Crate `opengdt-json`; hier nur zur Bequemlichkeit re-exportiert.
pub mod json {
    pub use opengdt_json::*;
}
pub use opengdt_json::Json;
pub use profile::{Key, Meta, Profile, SopClass, StorageContext, TransferSyntax};

/// Schema-Kennung, die `meta.schema` tragen muss.
pub const SCHEMA: &str = "opengdt-device-profile/1";
