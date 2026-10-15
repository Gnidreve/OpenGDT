//! Minimaler AVI-Writer (RIFF „AVI “ 1.0, genau ein Videostream, Index `idx1`).
//!
//! Zwei Arten von Frames:
//! * **MJPG**: fertige JPEG-Baseline-Bilder werden unverändert eingepackt (verlustfrei).
//! * **Unkomprimiert** (`Gray8` mit Graupalette, `Bgr24`): DIB-Zeilen, bottom-up, auf 4 Byte gepolstert.
//!
//! Grenze: klassisches AVI, höchstens 2 GiB – größere Clips liefern `Error::Unsupported`.

use crate::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Codec {
    Mjpeg,
    Gray8,
    Bgr24,
}

pub struct Video<'a> {
    pub width: u32,
    pub height: u32,
    /// Dauer eines Bildes in Mikrosekunden.
    pub frame_us: u32,
    pub codec: Codec,
    pub frames: &'a [Vec<u8>],
}

const MAX_FILE: u64 = 0x7FFF_FFFF;

fn le16(o: &mut Vec<u8>, x: u16) {
    o.extend_from_slice(&x.to_le_bytes());
}
fn le32(o: &mut Vec<u8>, x: u32) {
    o.extend_from_slice(&x.to_le_bytes());
}

/// RIFF-Chunk: Kennung, Länge, Daten, Füllbyte auf gerade Länge.
fn chunk(o: &mut Vec<u8>, id: &[u8; 4], data: &[u8]) {
    o.extend_from_slice(id);
    le32(o, data.len() as u32);
    o.extend_from_slice(data);
    if data.len() % 2 == 1 {
        o.push(0);
    }
}

fn list(o: &mut Vec<u8>, kind: &[u8; 4], body: &[u8]) {
    o.extend_from_slice(b"LIST");
    le32(o, (body.len() + 4) as u32);
    o.extend_from_slice(kind);
    o.extend_from_slice(body);
}

/// Bildzeilen (oben nach unten, `bytes_per_pixel` je Pixel) → DIB: unten nach oben, Zeilen auf 4 Byte gepolstert.
pub fn dib_rows(width: usize, height: usize, bytes_per_pixel: usize, top_down: &[u8]) -> Vec<u8> {
    let row = width * bytes_per_pixel;
    let stride = row.div_ceil(4) * 4;
    let mut out = vec![0u8; stride * height];
    for y in 0..height {
        let src = &top_down[y * row..(y + 1) * row];
        let dst = (height - 1 - y) * stride;
        out[dst..dst + row].copy_from_slice(src);
    }
    out
}

pub fn write(v: &Video) -> Result<Vec<u8>, Error> {
    let n = v.frames.len();
    if n == 0 {
        return Err(Error::Invalid("Video ohne Frames".into()));
    }
    if v.width == 0 || v.height == 0 || v.frame_us == 0 {
        return Err(Error::Invalid(
            "Video mit Größe 0 oder ohne Bildrate".into(),
        ));
    }
    let total: u64 = v
        .frames
        .iter()
        .map(|f| f.len() as u64 + 9 + 16)
        .sum::<u64>()
        + 4096;
    if total > MAX_FILE {
        return Err(Error::Unsupported(
            "Clip zu groß für AVI (über 2 GiB)".into(),
        ));
    }
    let max_size = v.frames.iter().map(Vec::len).max().unwrap_or(0) as u32;
    let (bits, handler, compression, ckid): (u16, &[u8; 4], &[u8; 4], &[u8; 4]) = match v.codec {
        Codec::Mjpeg => (24, b"MJPG", b"MJPG", b"00dc"),
        Codec::Gray8 => (8, b"DIB ", &[0; 4], b"00db"),
        Codec::Bgr24 => (24, b"DIB ", &[0; 4], b"00db"),
    };

    // Hauptheader
    let mut avih = Vec::new();
    le32(&mut avih, v.frame_us);
    le32(
        &mut avih,
        (max_size as u64 * 1_000_000 / v.frame_us as u64).min(u32::MAX as u64) as u32,
    );
    le32(&mut avih, 0); // Padding
    le32(&mut avih, 0x10); // AVIF_HASINDEX
    le32(&mut avih, n as u32);
    le32(&mut avih, 0);
    le32(&mut avih, 1); // Streams
    le32(&mut avih, max_size);
    le32(&mut avih, v.width);
    le32(&mut avih, v.height);
    avih.extend_from_slice(&[0; 16]);

    // Stream-Header
    let mut strh = Vec::new();
    strh.extend_from_slice(b"vids");
    strh.extend_from_slice(handler);
    le32(&mut strh, 0); // Flags
    le16(&mut strh, 0); // Priority
    le16(&mut strh, 0); // Language
    le32(&mut strh, 0); // InitialFrames
    le32(&mut strh, v.frame_us); // Scale
    le32(&mut strh, 1_000_000); // Rate → Bilder/s = Rate/Scale
    le32(&mut strh, 0); // Start
    le32(&mut strh, n as u32); // Length
    le32(&mut strh, max_size);
    le32(&mut strh, 0xFFFF_FFFF); // Quality
    le32(&mut strh, 0); // SampleSize
    le16(&mut strh, 0);
    le16(&mut strh, 0);
    le16(&mut strh, v.width.min(i16::MAX as u32) as u16);
    le16(&mut strh, v.height.min(i16::MAX as u32) as u16);

    // Stream-Format: BITMAPINFOHEADER (+ Graupalette)
    let mut strf = Vec::new();
    le32(&mut strf, 40);
    le32(&mut strf, v.width);
    le32(&mut strf, v.height); // positiv: bottom-up (bei MJPG unerheblich)
    le16(&mut strf, 1);
    le16(&mut strf, bits);
    strf.extend_from_slice(compression);
    le32(&mut strf, max_size);
    le32(&mut strf, 0);
    le32(&mut strf, 0);
    le32(&mut strf, if v.codec == Codec::Gray8 { 256 } else { 0 });
    le32(&mut strf, 0);
    if v.codec == Codec::Gray8 {
        for i in 0..=255u8 {
            strf.extend_from_slice(&[i, i, i, 0]);
        }
    }

    let mut strl = Vec::new();
    chunk(&mut strl, b"strh", &strh);
    chunk(&mut strl, b"strf", &strf);
    let mut hdrl = Vec::new();
    chunk(&mut hdrl, b"avih", &avih);
    list(&mut hdrl, b"strl", &strl);

    // Daten + Index (Offsets zählen ab dem „movi“-Kennwort)
    let mut movi = Vec::new();
    let mut idx = Vec::new();
    for f in v.frames {
        let off = (movi.len() + 4) as u32;
        chunk(&mut movi, ckid, f);
        idx.extend_from_slice(ckid);
        le32(&mut idx, 0x10); // AVIIF_KEYFRAME
        le32(&mut idx, off);
        le32(&mut idx, f.len() as u32);
    }

    let mut body = b"AVI ".to_vec();
    list(&mut body, b"hdrl", &hdrl);
    list(&mut body, b"movi", &movi);
    chunk(&mut body, b"idx1", &idx);
    let mut out = b"RIFF".to_vec();
    le32(&mut out, body.len() as u32);
    out.extend_from_slice(&body);
    Ok(out)
}

/// Breite und Höhe aus dem ersten SOF-Marker eines JPEG-Datenstroms.
pub fn jpeg_dimensions(j: &[u8]) -> Option<(u32, u32)> {
    if j.len() < 4 || j[0] != 0xFF || j[1] != 0xD8 {
        return None;
    }
    let mut p = 2;
    while p + 4 <= j.len() {
        if j[p] != 0xFF {
            p += 1;
            continue;
        }
        let m = j[p + 1];
        match m {
            0xFF => p += 1, // Füll-FF
            0x00 | 0x01 | 0xD0..=0xD8 => p += 2,
            0xD9 | 0xDA => return None, // Bildende/Scan ohne SOF davor
            0xC0..=0xCF if !matches!(m, 0xC4 | 0xC8 | 0xCC) => {
                let b = j.get(p + 4..p + 9)?;
                let h = u16::from_be_bytes([b[1], b[2]]) as u32;
                let w = u16::from_be_bytes([b[3], b[4]]) as u32;
                return (w > 0 && h > 0).then_some((w, h));
            }
            _ => {
                let len = u16::from_be_bytes([*j.get(p + 2)?, *j.get(p + 3)?]) as usize;
                p += 2 + len.max(2);
            }
        }
    }
    None
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// (Position, Größe) der Frame-Chunks und (Offset, Größe) der idx1-Einträge.
    pub type Walked = (Vec<(usize, usize)>, Vec<(u32, u32)>);

    fn u32_at(b: &[u8], p: usize) -> u32 {
        u32::from_le_bytes(b[p..p + 4].try_into().unwrap())
    }

    /// Läuft die RIFF-Struktur ab und prüft alle Längen. Gibt (Frame-Chunks, idx1-Einträge) zurück.
    pub fn walk(avi: &[u8]) -> Walked {
        assert_eq!(&avi[..4], b"RIFF");
        assert_eq!(u32_at(avi, 4) as usize, avi.len() - 8, "RIFF-Größe");
        assert_eq!(&avi[8..12], b"AVI ");
        let mut p = 12;
        let (mut frames, mut index) = (Vec::new(), Vec::new());
        let mut movi_tag = 0;
        while p < avi.len() {
            let id = &avi[p..p + 4];
            let size = u32_at(avi, p + 4) as usize;
            assert!(
                p + 8 + size <= avi.len(),
                "Chunk {:?} ragt über das Dateiende",
                id
            );
            match id {
                b"LIST" => {
                    let kind = &avi[p + 8..p + 12];
                    if kind == b"movi" {
                        movi_tag = p + 8;
                        let mut q = p + 12;
                        while q < p + 8 + size {
                            let cs = u32_at(avi, q + 4) as usize;
                            frames.push((q, cs));
                            q += 8 + cs + cs % 2;
                        }
                        assert_eq!(q, p + 8 + size, "movi-Inhalt füllt die Liste exakt");
                    }
                }
                b"idx1" => {
                    for e in avi[p + 8..p + 8 + size].chunks(16) {
                        index.push((u32_at(e, 8), u32_at(e, 12)));
                    }
                }
                _ => {}
            }
            p += 8 + size + size % 2;
        }
        assert_eq!(p, avi.len());
        // Index-Offsets müssen auf die Frame-Chunks zeigen (relativ zum „movi“-Kennwort)
        for ((off, size), (pos, cs)) in index.iter().zip(&frames) {
            assert_eq!(movi_tag + *off as usize, *pos);
            assert_eq!(*size as usize, *cs);
        }
        (frames, index)
    }

    fn video(codec: Codec, frames: &[Vec<u8>]) -> Vec<u8> {
        write(&Video {
            width: 4,
            height: 2,
            frame_us: 40_000,
            codec,
            frames,
        })
        .unwrap()
    }

    #[test]
    fn structure_is_consistent_for_all_codecs() {
        let frames = vec![vec![1u8; 11], vec![2u8; 8], vec![3u8; 1]]; // ungerade Längen → Padding
        for codec in [Codec::Mjpeg, Codec::Gray8, Codec::Bgr24] {
            let avi = video(codec, &frames);
            let (f, idx) = walk(&avi);
            assert_eq!((f.len(), idx.len()), (3, 3), "{codec:?}");
        }
    }

    #[test]
    fn header_fields() {
        let avi = video(Codec::Mjpeg, &[vec![0; 10], vec![0; 10]]);
        let avih = avi.windows(4).position(|w| w == b"avih").unwrap() + 8;
        assert_eq!(u32_at(&avi, avih), 40_000, "µs je Bild");
        assert_eq!(u32_at(&avi, avih + 16), 2, "Frames");
        assert_eq!(u32_at(&avi, avih + 32), 4, "Breite");
        assert_eq!(u32_at(&avi, avih + 36), 2, "Höhe");
        assert!(avi.windows(4).any(|w| w == b"MJPG"));
    }

    #[test]
    fn rejects_bad_input() {
        let ok = |w, h, us| {
            write(&Video {
                width: w,
                height: h,
                frame_us: us,
                codec: Codec::Mjpeg,
                frames: &[vec![1]],
            })
        };
        assert!(ok(0, 2, 1).is_err() && ok(2, 0, 1).is_err() && ok(2, 2, 0).is_err());
        assert!(write(&Video {
            width: 1,
            height: 1,
            frame_us: 1,
            codec: Codec::Mjpeg,
            frames: &[]
        })
        .is_err());
    }

    #[test]
    fn dib_rows_flip_and_pad() {
        // 3 Pixel breit, 2 Zeilen, 1 Byte/Pixel → Stride 4, unten zuerst
        let d = dib_rows(3, 2, 1, &[1, 2, 3, 4, 5, 6]);
        assert_eq!(d, vec![4, 5, 6, 0, 1, 2, 3, 0]);
        assert_eq!(dib_rows(1, 1, 3, &[9, 8, 7]).len(), 4);
    }

    #[test]
    fn jpeg_dimensions_from_sof() {
        // SOI, APP0 (Länge 4), SOF0: Länge 11, 8 Bit, Höhe 0x0040, Breite 0x0060
        let j = [
            0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0x00, 0x00, 0xFF, 0xC0, 0x00, 0x0B, 0x08, 0x00,
            0x40, 0x00, 0x60, 1, 1, 0x11, 0,
        ];
        assert_eq!(jpeg_dimensions(&j), Some((96, 64)));
        assert_eq!(jpeg_dimensions(&[0xFF, 0xD8, 0xFF, 0xD9]), None);
        assert_eq!(jpeg_dimensions(b"kein jpeg"), None);
        assert_eq!(jpeg_dimensions(&j[..10]), None, "abgeschnitten");
        // DHT (C4) wird nicht für ein SOF gehalten
        let k = [
            0xFF, 0xD8, 0xFF, 0xC4, 0x00, 0x02, 0xFF, 0xC0, 0x00, 0x0B, 0x08, 0x00, 0x10, 0x00,
            0x20, 1, 1, 0x11, 0,
        ];
        assert_eq!(jpeg_dimensions(&k), Some((32, 16)));
    }
}
