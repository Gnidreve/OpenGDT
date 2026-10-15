//! DICOM Part 10-Dateien (Preamble + `DICM` + File Meta + Datensatz).

use crate::dataset::Dataset;
use crate::tag::{t, Vr};
use crate::{uid, Error};

/// Ein gelesener Part-10-Datei-Inhalt.
pub struct Part10 {
    pub meta: Dataset,
    pub transfer_syntax: String,
    pub sop_class: String,
    pub sop_instance: String,
    /// Der Datensatz *nach* der Meta-Gruppe, roh in `transfer_syntax` kodiert.
    pub data: Vec<u8>,
}

pub fn write_part10(
    sop_class: &str,
    sop_instance: &str,
    transfer_syntax: &str,
    data: &[u8],
) -> Vec<u8> {
    let mut meta = Dataset::new();
    meta.set_bytes(t::META_VERSION, Vr::OB, vec![0, 1]);
    meta.set_str(t::META_SOP_CLASS_UID, sop_class);
    meta.set_str(t::META_SOP_INSTANCE_UID, sop_instance);
    meta.set_str(t::META_TRANSFER_SYNTAX, transfer_syntax);
    meta.set_str(t::META_IMPL_CLASS_UID, uid::IMPLEMENTATION_CLASS);
    meta.set_str(t::META_IMPL_VERSION_NAME, uid::IMPLEMENTATION_VERSION);
    let body = meta.encode(true);
    let mut group_len = Dataset::new();
    group_len.set_u32(t::META_GROUP_LENGTH, body.len() as u32);
    let mut out = vec![0u8; 128];
    out.extend_from_slice(b"DICM");
    out.extend_from_slice(&group_len.encode(true));
    out.extend_from_slice(&body);
    out.extend_from_slice(data);
    out
}

pub fn read_part10(bytes: &[u8]) -> Result<Part10, Error> {
    if bytes.len() < 132 || &bytes[128..132] != b"DICM" {
        return Err(Error::Invalid("keine Part-10-Datei"));
    }
    let rest = &bytes[132..];
    let (meta, used) = Dataset::decode_until_group_ends(rest, true, 0x0002)?;
    let get = |tag| {
        meta.get_str(tag)
            .ok_or(Error::Invalid("File-Meta unvollständig"))
    };
    Ok(Part10 {
        transfer_syntax: get(t::META_TRANSFER_SYNTAX)?,
        sop_class: get(t::META_SOP_CLASS_UID)?,
        sop_instance: get(t::META_SOP_INSTANCE_UID)?,
        data: rest[used..].to_vec(),
        meta,
    })
}

/// Datensatz-Kodierung für eine Transfer Syntax: `Some(explicit)` oder `None`
/// bei nicht unterstützten (Big Endian, Deflate).
pub fn is_explicit(transfer_syntax: &str) -> Option<bool> {
    match transfer_syntax {
        uid::IMPLICIT_VR_LE => Some(false),
        uid::EXPLICIT_VR_BE | uid::DEFLATED_EXPLICIT_VR_LE => None,
        _ => Some(true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn part10_roundtrip() {
        let mut ds = Dataset::new();
        ds.set_str(t::PATIENT_ID, "4711");
        let bytes = write_part10(
            "1.2.840.10008.5.1.4.1.1.6.1",
            "1.2.3.4",
            uid::EXPLICIT_VR_LE,
            &ds.encode(true),
        );
        let p = read_part10(&bytes).unwrap();
        assert_eq!(p.sop_instance, "1.2.3.4");
        assert_eq!(p.transfer_syntax, uid::EXPLICIT_VR_LE);
        let back = Dataset::decode(&p.data, true).unwrap();
        assert_eq!(back.get_str(t::PATIENT_ID).as_deref(), Some("4711"));
    }

    #[test]
    fn rejects_garbage() {
        assert!(read_part10(&[0u8; 200]).is_err());
        assert!(read_part10(b"short").is_err());
    }
}
