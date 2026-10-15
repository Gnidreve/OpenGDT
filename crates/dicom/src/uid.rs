//! UID-Konstanten und UID-Erzeugung.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};

pub const VERIFICATION: &str = "1.2.840.10008.1.1";
pub const MWL_FIND: &str = "1.2.840.10008.5.1.4.31";
pub const IMPLICIT_VR_LE: &str = "1.2.840.10008.1.2";
pub const EXPLICIT_VR_LE: &str = "1.2.840.10008.1.2.1";
pub const DEFLATED_EXPLICIT_VR_LE: &str = "1.2.840.10008.1.2.1.99";
pub const EXPLICIT_VR_BE: &str = "1.2.840.10008.1.2.2";
pub const APPLICATION_CONTEXT: &str = "1.2.840.10008.3.1.1.1";
pub const STORAGE_PREFIX: &str = "1.2.840.10008.5.1.4.1.1.";

/// Implementation Class UID dieses Projekts (2.25.-Root, zufällig einmalig gewählt).
pub const IMPLEMENTATION_CLASS: &str = "2.25.186952739502964110576013028569317203871";
pub const IMPLEMENTATION_VERSION: &str = "OPENGDT_0_1";

/// Encapsulated-Transfer-Syntaxes, deren Fragmente direkt als Datei taugen.
pub const JPEG_BASELINE: &str = "1.2.840.10008.1.2.4.50";
pub const MPEG4_HP41: &str = "1.2.840.10008.1.2.4.102";
pub const MPEG4_HP41_BD: &str = "1.2.840.10008.1.2.4.103";
pub const MPEG4_HP42_2D: &str = "1.2.840.10008.1.2.4.104";
pub const MPEG4_HP42_3D: &str = "1.2.840.10008.1.2.4.105";
pub const MPEG4_STEREO: &str = "1.2.840.10008.1.2.4.106";

pub fn is_storage_sop_class(uid: &str) -> bool {
    uid.starts_with(STORAGE_PREFIX)
}

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// Neue UID unter `2.25.<128-Bit-Zahl>`. Entropie aus `RandomState` (vom OS
/// zufällig geseedet), Zeit und Zähler – kein externer RNG nötig.
pub fn generate() -> String {
    let mut h1 = RandomState::new().build_hasher();
    h1.write_u64(COUNTER.fetch_add(1, Ordering::Relaxed));
    let mut h2 = RandomState::new().build_hasher();
    h2.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
    );
    let n = ((h1.finish() as u128) << 64) | h2.finish() as u128;
    format!("2.25.{n}")
}

#[cfg(test)]
mod tests {
    #[test]
    fn uids_are_unique_and_short_enough() {
        let a = super::generate();
        let b = super::generate();
        assert_ne!(a, b);
        assert!(a.len() <= 64);
    }
}
