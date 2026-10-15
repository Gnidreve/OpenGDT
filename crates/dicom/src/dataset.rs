//! DICOM-Datensätze: Aufbau, Encoding und Decoding (Implicit/Explicit VR Little Endian).

use crate::tag::{self, t, vr_of, Tag, Vr};
use crate::Error;
use std::collections::BTreeMap;

const UNDEFINED: u32 = 0xFFFF_FFFF;

/// Maximale Verschachtelungstiefe von Sequenzen. Echte Datensätze liegen bei höchstens 5–6; ohne Limit
/// könnte ein präparierter Datensatz aus dem Netzwerk den Stack überlaufen lassen (Prozessabbruch).
const MAX_NESTING: usize = 32;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Bytes(Vec<u8>),
    Sequence(Vec<Dataset>),
    /// Encapsulated Pixel Data: Fragmente (das erste ist die Basic Offset Table).
    Fragments(Vec<Vec<u8>>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Element {
    pub tag: Tag,
    pub vr: Vr,
    pub value: Value,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Dataset {
    elements: BTreeMap<Tag, Element>,
}

impl Dataset {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Element> {
        self.elements.values()
    }

    pub fn insert(&mut self, e: Element) {
        self.elements.insert(e.tag, e);
    }

    pub fn get(&self, tag: Tag) -> Option<&Element> {
        self.elements.get(&tag)
    }

    pub fn remove(&mut self, tag: Tag) -> Option<Element> {
        self.elements.remove(&tag)
    }

    pub fn contains(&self, tag: Tag) -> bool {
        self.elements.contains_key(&tag)
    }

    // ---- Setter -------------------------------------------------------

    /// Textwert; VR kommt aus dem Wörterbuch (unbekannt → `LO`).
    pub fn set_str(&mut self, tag: Tag, value: &str) {
        let vr = match vr_of(tag) {
            Vr::UN => Vr::LO,
            v => v,
        };
        self.set_str_vr(tag, vr, value.as_bytes().to_vec());
    }

    /// Textwert als bereits kodierte Bytes (z. B. Latin-1).
    pub fn set_str_vr(&mut self, tag: Tag, vr: Vr, bytes: Vec<u8>) {
        self.insert(Element {
            tag,
            vr,
            value: Value::Bytes(bytes),
        });
    }

    pub fn set_u16(&mut self, tag: Tag, v: u16) {
        self.insert(Element {
            tag,
            vr: Vr::US,
            value: Value::Bytes(v.to_le_bytes().to_vec()),
        });
    }

    pub fn set_u32(&mut self, tag: Tag, v: u32) {
        self.insert(Element {
            tag,
            vr: Vr::UL,
            value: Value::Bytes(v.to_le_bytes().to_vec()),
        });
    }

    pub fn set_bytes(&mut self, tag: Tag, vr: Vr, bytes: Vec<u8>) {
        self.insert(Element {
            tag,
            vr,
            value: Value::Bytes(bytes),
        });
    }

    pub fn set_seq(&mut self, tag: Tag, items: Vec<Dataset>) {
        self.insert(Element {
            tag,
            vr: Vr::SQ,
            value: Value::Sequence(items),
        });
    }

    // ---- Getter -------------------------------------------------------

    pub fn get_bytes(&self, tag: Tag) -> Option<&[u8]> {
        match &self.get(tag)?.value {
            Value::Bytes(b) => Some(b),
            _ => None,
        }
    }

    /// Text ohne Padding. Zeichensatz: UTF-8 falls gültig, sonst Latin-1
    /// (deckt `ISO_IR 100` und `ISO_IR 192` ab, andere Sätze werden nicht unterstützt).
    pub fn get_str(&self, tag: Tag) -> Option<String> {
        let b = self.get_bytes(tag)?;
        let s = match std::str::from_utf8(b) {
            Ok(s) => s.to_string(),
            Err(_) => b.iter().map(|&c| c as char).collect(),
        };
        Some(s.trim_end_matches(['\0', ' ']).to_string())
    }

    pub fn get_u16(&self, tag: Tag) -> Option<u16> {
        let b = self.get_bytes(tag)?;
        Some(u16::from_le_bytes(b.get(..2)?.try_into().ok()?))
    }

    pub fn get_u32(&self, tag: Tag) -> Option<u32> {
        let b = self.get_bytes(tag)?;
        Some(u32::from_le_bytes(b.get(..4)?.try_into().ok()?))
    }

    pub fn get_seq(&self, tag: Tag) -> Option<&[Dataset]> {
        match &self.get(tag)?.value {
            Value::Sequence(s) => Some(s),
            _ => None,
        }
    }

    // ---- Encoding -----------------------------------------------------

    pub fn encode(&self, explicit: bool) -> Vec<u8> {
        let mut out = Vec::new();
        self.encode_into(&mut out, explicit);
        out
    }

    fn encode_into(&self, out: &mut Vec<u8>, explicit: bool) {
        for e in self.elements.values() {
            out.extend_from_slice(&e.tag.0.to_le_bytes());
            out.extend_from_slice(&e.tag.1.to_le_bytes());
            match &e.value {
                Value::Bytes(b) => {
                    let pad = b.len() % 2;
                    let len = (b.len() + pad) as u32;
                    write_header(out, e.vr, len, explicit);
                    out.extend_from_slice(b);
                    if pad == 1 {
                        out.push(e.vr.pad_byte());
                    }
                }
                Value::Sequence(items) => {
                    let mut body = Vec::new();
                    for it in items {
                        let enc = it.encode(explicit);
                        body.extend_from_slice(&tag::ITEM.0.to_le_bytes());
                        body.extend_from_slice(&tag::ITEM.1.to_le_bytes());
                        body.extend_from_slice(&(enc.len() as u32).to_le_bytes());
                        body.extend_from_slice(&enc);
                    }
                    write_header(out, Vr::SQ, body.len() as u32, explicit);
                    out.extend_from_slice(&body);
                }
                Value::Fragments(frags) => {
                    // Encapsulated nur in Explicit VR sinnvoll.
                    write_header(out, e.vr, UNDEFINED, explicit);
                    for f in frags {
                        out.extend_from_slice(&tag::ITEM.0.to_le_bytes());
                        out.extend_from_slice(&tag::ITEM.1.to_le_bytes());
                        let pad = f.len() % 2;
                        out.extend_from_slice(&((f.len() + pad) as u32).to_le_bytes());
                        out.extend_from_slice(f);
                        if pad == 1 {
                            out.push(0);
                        }
                    }
                    out.extend_from_slice(&tag::SEQ_DELIM.0.to_le_bytes());
                    out.extend_from_slice(&tag::SEQ_DELIM.1.to_le_bytes());
                    out.extend_from_slice(&0u32.to_le_bytes());
                }
            }
        }
    }

    // ---- Decoding -----------------------------------------------------

    pub fn decode(data: &[u8], explicit: bool) -> Result<Dataset, Error> {
        let mut r = Reader { d: data, pos: 0 };
        decode_elements(&mut r, explicit, data.len(), false, 0)
    }

    /// Wie `decode`, liefert aber auch die Position, an der gelesen wurde
    /// (für File-Meta-Gruppe, die vor dem eigentlichen Datensatz steht).
    pub fn decode_until_group_ends(
        data: &[u8],
        explicit: bool,
        group: u16,
    ) -> Result<(Dataset, usize), Error> {
        let mut pos = 0;
        while pos + 4 <= data.len() {
            let g = u16::from_le_bytes([data[pos], data[pos + 1]]);
            if g != group {
                break;
            }
            // Element überspringen, um die Gruppengrenze zu finden
            let mut r = Reader { d: data, pos };
            read_element_header(&mut r, explicit)?;
            let (_, _, len) = read_element_header_peek(data, pos, explicit)?;
            pos = r.pos + len as usize;
        }
        let ds = Dataset::decode(&data[..pos.min(data.len())], explicit)?;
        Ok((ds, pos))
    }
}

fn write_header(out: &mut Vec<u8>, vr: Vr, len: u32, explicit: bool) {
    if explicit {
        out.extend_from_slice(&vr.0);
        if vr.has_long_length() {
            out.extend_from_slice(&[0, 0]);
            out.extend_from_slice(&len.to_le_bytes());
        } else {
            out.extend_from_slice(&(len as u16).to_le_bytes());
        }
    } else {
        out.extend_from_slice(&len.to_le_bytes());
    }
}

struct Reader<'a> {
    d: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let end = self.pos.checked_add(n).ok_or(Error::Truncated)?;
        if end > self.d.len() {
            return Err(Error::Truncated);
        }
        let s = &self.d[self.pos..end];
        self.pos = end;
        Ok(s)
    }
    fn u16(&mut self) -> Result<u16, Error> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }
    fn u32(&mut self) -> Result<u32, Error> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn tag(&mut self) -> Result<Tag, Error> {
        let g = self.u16()?;
        let e = self.u16()?;
        Ok(Tag(g, e))
    }
}

/// (Tag, VR, Länge) lesen.
fn read_element_header(r: &mut Reader, explicit: bool) -> Result<(Tag, Vr, u32), Error> {
    let tag = r.tag()?;
    if tag.0 == 0xFFFE {
        return Ok((tag, Vr::UN, r.u32()?));
    }
    if explicit {
        let b = r.take(2)?;
        let vr = Vr([b[0], b[1]]);
        if !vr.is_valid() {
            return Err(Error::Invalid("ungültiges VR"));
        }
        let len = if vr.has_long_length() {
            r.take(2)?;
            r.u32()?
        } else {
            r.u16()? as u32
        };
        Ok((tag, vr, len))
    } else {
        let len = r.u32()?;
        Ok((tag, vr_of(tag), len))
    }
}

fn read_element_header_peek(
    data: &[u8],
    pos: usize,
    explicit: bool,
) -> Result<(Tag, Vr, u32), Error> {
    let mut r = Reader { d: data, pos };
    read_element_header(&mut r, explicit)
}

/// Liest Elemente bis `end` bzw. (bei `until_item_delim`) bis zum Item-Delimiter.
fn decode_elements(
    r: &mut Reader,
    explicit: bool,
    end: usize,
    until_item_delim: bool,
    depth: usize,
) -> Result<Dataset, Error> {
    let mut ds = Dataset::new();
    loop {
        if r.pos >= end {
            if until_item_delim {
                return Err(Error::Truncated);
            }
            break;
        }
        let (tag, vr, len) = read_element_header(r, explicit)?;
        if tag == tag::ITEM_DELIM {
            if until_item_delim {
                break;
            }
            return Err(Error::Invalid("unerwarteter Item-Delimiter"));
        }
        if tag.0 == 0xFFFE {
            return Err(Error::Invalid("unerwartetes Item/Delimiter-Tag"));
        }
        // Implicit VR: Sequenz-Tags, die das kleine Wörterbuch nicht kennt, erkennt man
        // am Item-Marker direkt am Wertanfang (übliche Heuristik anderer Toolkits).
        let looks_like_seq = !explicit
            && vr == Vr::UN
            && len != UNDEFINED
            && len >= 8
            && r.d.get(r.pos..r.pos + 4) == Some(&[0xFE, 0xFF, 0x00, 0xE0][..]);
        let is_seq =
            vr == Vr::SQ || looks_like_seq || (!explicit && len == UNDEFINED && vr == Vr::UN);
        let value = if is_seq {
            Value::Sequence(decode_sequence(r, explicit, len, depth)?)
        } else if len == UNDEFINED {
            Value::Fragments(decode_fragments(r)?)
        } else {
            if r.pos + len as usize > end {
                return Err(Error::Truncated);
            }
            Value::Bytes(r.take(len as usize)?.to_vec())
        };
        let vr = if is_seq { Vr::SQ } else { vr };
        ds.insert(Element { tag, vr, value });
    }
    Ok(ds)
}

fn decode_sequence(
    r: &mut Reader,
    explicit: bool,
    len: u32,
    depth: usize,
) -> Result<Vec<Dataset>, Error> {
    if depth >= MAX_NESTING {
        return Err(Error::Invalid("Sequenzen zu tief verschachtelt"));
    }
    let mut items = Vec::new();
    let end = if len == UNDEFINED {
        usize::MAX
    } else {
        r.pos + len as usize
    };
    if end != usize::MAX && end > r.d.len() {
        return Err(Error::Truncated);
    }
    loop {
        if end != usize::MAX && r.pos >= end {
            break;
        }
        let tag = r.tag()?;
        let ilen = r.u32()?;
        if tag == tag::SEQ_DELIM {
            break;
        }
        if tag != tag::ITEM {
            return Err(Error::Invalid("Sequenz enthält kein Item"));
        }
        let ds = if ilen == UNDEFINED {
            decode_elements(r, explicit, r.d.len(), true, depth + 1)?
        } else {
            let iend = r.pos + ilen as usize;
            if iend > r.d.len() {
                return Err(Error::Truncated);
            }
            decode_elements(r, explicit, iend, false, depth + 1)?
        };
        items.push(ds);
    }
    Ok(items)
}

fn decode_fragments(r: &mut Reader) -> Result<Vec<Vec<u8>>, Error> {
    let mut frags = Vec::new();
    loop {
        let tag = r.tag()?;
        let len = r.u32()?;
        if tag == tag::SEQ_DELIM {
            break;
        }
        if tag != tag::ITEM || len == UNDEFINED {
            return Err(Error::Invalid("fehlerhafte Fragmente"));
        }
        frags.push(r.take(len as usize)?.to_vec());
    }
    Ok(frags)
}

/// Bequemer Builder für Command-Datensätze.
pub fn command_group_length(ds: &Dataset) -> u32 {
    // Länge aller Elemente außer (0000,0000) selbst, Implicit VR LE.
    let mut copy = ds.clone();
    copy.remove(t::COMMAND_GROUP_LENGTH);
    copy.encode(false).len() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Dataset {
        let mut item = Dataset::new();
        item.set_str(t::MODALITY, "US");
        item.set_str(t::SPS_STATION_AE, "MINDRAY");
        let mut ds = Dataset::new();
        ds.set_str(t::PATIENT_NAME, "Müller^Jürgen");
        ds.set_str(t::PATIENT_ID, "4711");
        ds.set_u16(t::ROWS, 480);
        ds.set_seq(t::SPS_SEQUENCE, vec![item]);
        ds
    }

    #[test]
    fn roundtrip_both_syntaxes() {
        for explicit in [false, true] {
            let ds = sample();
            let back = Dataset::decode(&ds.encode(explicit), explicit).unwrap();
            assert_eq!(back.get_str(t::PATIENT_ID).as_deref(), Some("4711"));
            assert_eq!(back.get_u16(t::ROWS), Some(480));
            let sps = back.get_seq(t::SPS_SEQUENCE).unwrap();
            assert_eq!(sps[0].get_str(t::MODALITY).as_deref(), Some("US"));
        }
    }

    #[test]
    fn odd_length_is_padded_and_trimmed() {
        let mut ds = Dataset::new();
        ds.set_str(t::PATIENT_ID, "123");
        ds.set_str(t::SOP_CLASS_UID, "1.2.3");
        let enc = ds.encode(true);
        assert_eq!(enc.len() % 2, 0);
        let back = Dataset::decode(&enc, true).unwrap();
        assert_eq!(back.get_str(t::PATIENT_ID).as_deref(), Some("123"));
        assert_eq!(back.get_str(t::SOP_CLASS_UID).as_deref(), Some("1.2.3"));
    }

    #[test]
    fn undefined_length_sequence_and_item() {
        // (0040,0100) SQ undefined length, ein Item undefined length mit (0008,0060) "US"
        let mut b = Vec::new();
        b.extend([0x40, 0x00, 0x00, 0x01, 0xFF, 0xFF, 0xFF, 0xFF]); // implicit: tag + undefined
        b.extend([0xFE, 0xFF, 0x00, 0xE0, 0xFF, 0xFF, 0xFF, 0xFF]); // item undefined
        b.extend([0x08, 0x00, 0x60, 0x00, 0x02, 0x00, 0x00, 0x00, b'U', b'S']);
        b.extend([0xFE, 0xFF, 0x0D, 0xE0, 0, 0, 0, 0]); // item delim
        b.extend([0xFE, 0xFF, 0xDD, 0xE0, 0, 0, 0, 0]); // seq delim
        let ds = Dataset::decode(&b, false).unwrap();
        assert_eq!(
            ds.get_seq(t::SPS_SEQUENCE).unwrap()[0]
                .get_str(t::MODALITY)
                .as_deref(),
            Some("US")
        );
    }

    #[test]
    fn implicit_vr_unknown_tag_with_items_is_decoded_as_sequence() {
        // (0008,1120) ist nicht im Wörterbuch; Inhalt = ein Item mit (0008,1150) "1.2".
        let mut item = Vec::new();
        item.extend([
            0x08, 0x00, 0x50, 0x11, 0x04, 0x00, 0x00, 0x00, b'1', b'.', b'2', 0,
        ]);
        let mut b = vec![0x08, 0x00, 0x20, 0x11];
        b.extend(((8 + item.len()) as u32).to_le_bytes());
        b.extend([0xFE, 0xFF, 0x00, 0xE0]);
        b.extend((item.len() as u32).to_le_bytes());
        b.extend(&item);
        let ds = Dataset::decode(&b, false).unwrap();
        let seq = ds
            .get_seq(Tag(0x0008, 0x1120))
            .expect("als Sequenz erkannt");
        assert_eq!(seq.len(), 1);
        assert_eq!(seq[0].get_str(Tag(0x0008, 0x1150)).as_deref(), Some("1.2"));
        // Gewöhnliche Bytes, die zufällig kurz sind, bleiben Bytes.
        let plain = Dataset::decode(
            &[0x08, 0x00, 0x21, 0x11, 0x02, 0x00, 0x00, 0x00, b'A', b'B'],
            false,
        )
        .unwrap();
        assert!(plain.get_bytes(Tag(0x0008, 0x1121)).is_some());
    }

    #[test]
    fn encapsulated_fragments_roundtrip() {
        let mut ds = Dataset::new();
        ds.insert(Element {
            tag: t::PIXEL_DATA,
            vr: Vr::OB,
            value: Value::Fragments(vec![vec![], vec![1, 2, 3, 4], vec![5, 6]]),
        });
        let back = Dataset::decode(&ds.encode(true), true).unwrap();
        assert_eq!(back, ds);
    }

    #[test]
    fn truncated_input_is_error_not_panic() {
        let enc = sample().encode(true);
        for cut in 0..enc.len() {
            let _ = Dataset::decode(&enc[..cut], true);
            let _ = Dataset::decode(&enc[..cut], false);
        }
    }
}
