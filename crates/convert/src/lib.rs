//! Konvertiert DICOM-Instanzen in Dateien, die ein PVS anzeigen kann.
//!
//! Prinzip: *nichts neu kodieren, was schon in einem Standardformat vorliegt.*
//! JPEG-Baseline-Frames und MPEG-4-Streams stecken in DICOM unverändert
//! in den Fragmenten und werden nur ausgepackt. Unkomprimierte Pixeldaten
//! werden als PNG geschrieben (eigener Encoder, `png.rs`).

mod png;

use opengdt_dicom::dataset::Value;
use opengdt_dicom::file::is_explicit;
use opengdt_dicom::{t, uid, Dataset, Tag};

pub const ENCAPSULATED_DOCUMENT: Tag = Tag(0x0042, 0x0011);
pub const ENCAPSULATED_PDF_SOP_CLASS: &str = "1.2.840.10008.5.1.4.1.1.104.1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Still,
    Video,
    Document,
}

#[derive(Debug)]
pub struct Output {
    pub kind: Kind,
    /// Dateiendung ohne Punkt, z. B. `jpg`.
    pub extension: &'static str,
    /// Kurzformat für GDT (`JPG`, `MP4`, `PNG`, `PDF`).
    pub gdt_format: &'static str,
    pub bytes: Vec<u8>,
}

#[derive(Debug)]
pub enum Error {
    /// Transfer Syntax / Pixelformat wird (noch) nicht unterstützt.
    Unsupported(String),
    /// Daten sind inkonsistent.
    Invalid(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Unsupported(m) => write!(f, "nicht unterstützt: {m}"),
            Error::Invalid(m) => write!(f, "ungültig: {m}"),
        }
    }
}

impl std::error::Error for Error {}

#[derive(Clone, Copy, Debug, Default)]
pub struct Options {
    /// Bei Mehrbild-Instanzen ohne Videoformat alle Frames als Einzelbilder
    /// ausgeben (sonst nur den ersten).
    pub all_frames: bool,
}

/// `data` ist der rohe Datensatz in `transfer_syntax`.
pub fn convert(
    sop_class: &str,
    transfer_syntax: &str,
    data: &[u8],
    opts: Options,
) -> Result<Vec<Output>, Error> {
    let explicit = is_explicit(transfer_syntax)
        .ok_or_else(|| Error::Unsupported(format!("Transfer Syntax {transfer_syntax}")))?;
    let ds = Dataset::decode(data, explicit).map_err(|e| Error::Invalid(e.to_string()))?;
    convert_dataset(sop_class, transfer_syntax, &ds, opts)
}

pub fn convert_dataset(
    sop_class: &str,
    transfer_syntax: &str,
    ds: &Dataset,
    opts: Options,
) -> Result<Vec<Output>, Error> {
    if sop_class == ENCAPSULATED_PDF_SOP_CLASS {
        let doc = ds
            .get_bytes(ENCAPSULATED_DOCUMENT)
            .ok_or_else(|| Error::Invalid("Encapsulated Document fehlt".into()))?;
        let mut pdf = doc.to_vec();
        while pdf.last() == Some(&0) {
            pdf.pop();
        }
        return Ok(vec![Output {
            kind: Kind::Document,
            extension: "pdf",
            gdt_format: "PDF",
            bytes: pdf,
        }]);
    }
    let pixel = ds
        .get(t::PIXEL_DATA)
        .ok_or_else(|| Error::Unsupported("keine Pixeldaten (z. B. Structured Report)".into()))?;
    let frames: usize = ds
        .get_str(t::NUMBER_OF_FRAMES)
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(1)
        .max(1);

    match &pixel.value {
        Value::Fragments(frags) => convert_encapsulated(transfer_syntax, frags, frames, opts),
        Value::Bytes(raw) => {
            let geo = Geometry::from_dataset(ds)?;
            let count = if opts.all_frames { frames } else { 1 };
            let fsize = geo.frame_bytes();
            if raw.len() < fsize * frames.min(count.max(1)) {
                return Err(Error::Invalid("Pixeldaten kürzer als Bildgeometrie".into()));
            }
            (0..count)
                .map(|i| {
                    let frame = raw
                        .get(i * fsize..(i + 1) * fsize)
                        .ok_or_else(|| Error::Invalid("Frame außerhalb der Pixeldaten".into()))?;
                    Ok(Output {
                        kind: Kind::Still,
                        extension: "png",
                        gdt_format: "PNG",
                        bytes: geo.to_png(frame)?,
                    })
                })
                .collect()
        }
        Value::Sequence(_) => Err(Error::Invalid("Pixeldaten als Sequenz".into())),
    }
}

fn convert_encapsulated(
    ts: &str,
    frags: &[Vec<u8>],
    frames: usize,
    opts: Options,
) -> Result<Vec<Output>, Error> {
    // Erstes Fragment = Basic Offset Table.
    let data = frags.get(1..).unwrap_or(&[]);
    match ts {
        uid::JPEG_BASELINE => {
            let jpegs = split_frames(frags, frames)?;
            let take = if opts.all_frames { jpegs.len() } else { 1 };
            Ok(jpegs
                .into_iter()
                .take(take)
                .map(|bytes| Output { kind: Kind::Still, extension: "jpg", gdt_format: "JPG", bytes: strip_pad(bytes) })
                .collect())
        }
        uid::MPEG4_HP41 | uid::MPEG4_HP41_BD | uid::MPEG4_HP42_2D | uid::MPEG4_HP42_3D | uid::MPEG4_STEREO => {
            let bytes: Vec<u8> = data.iter().flatten().copied().collect();
            Ok(vec![Output { kind: Kind::Video, extension: "mp4", gdt_format: "MP4", bytes }])
        }
        other => Err(Error::Unsupported(format!(
            "komprimierte Transfer Syntax {other} (nur JPEG Baseline und MPEG-4 werden durchgereicht)"
        ))),
    }
}

/// Teilt Fragmente in Frames. Ohne verlässliche Offset-Tabelle gilt: 1 Frame →
/// alle Fragmente zusammen, sonst ein Fragment je Frame.
fn split_frames(frags: &[Vec<u8>], frames: usize) -> Result<Vec<Vec<u8>>, Error> {
    let data = frags.get(1..).unwrap_or(&[]);
    if data.is_empty() {
        return Err(Error::Invalid("keine Pixel-Fragmente".into()));
    }
    if frames == 1 {
        return Ok(vec![data.iter().flatten().copied().collect()]);
    }
    if data.len() == frames {
        return Ok(data.to_vec());
    }
    // Offset-Tabelle nutzen (4-Byte-LE-Offsets relativ zum ersten Fragment).
    let table = &frags[0];
    if table.len() == frames * 4 {
        let offsets: Vec<usize> = table
            .chunks(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]) as usize)
            .collect();
        let mut starts = Vec::new(); // Fragmentindex je Frame
        let mut pos = 0usize;
        let mut idx = 0usize;
        for off in &offsets {
            while pos < *off && idx < data.len() {
                pos += 8 + data[idx].len();
                idx += 1;
            }
            if pos != *off {
                return Err(Error::Invalid(
                    "Offset-Tabelle passt nicht zu den Fragmenten".into(),
                ));
            }
            starts.push(idx);
        }
        starts.push(data.len());
        return Ok(starts
            .windows(2)
            .map(|w| data[w[0]..w[1]].iter().flatten().copied().collect())
            .collect());
    }
    Err(Error::Unsupported(
        "Fragment-Zuordnung zu Frames nicht eindeutig".into(),
    ))
}

/// DICOM füllt ungerade JPEG-Längen mit einem Nullbyte auf; hinter dem EOI-Marker
/// (FFD9) gehört nichts mehr zum Bild.
fn strip_pad(mut v: Vec<u8>) -> Vec<u8> {
    while v.len() >= 2 && v[v.len() - 2..] != [0xFF, 0xD9] && v.last() == Some(&0) {
        v.pop();
    }
    v
}

struct Geometry {
    rows: usize,
    cols: usize,
    bits: usize,
    samples: usize,
    planar: bool,
    mono1: bool,
    ybr: bool,
}

impl Geometry {
    fn from_dataset(ds: &Dataset) -> Result<Geometry, Error> {
        let get = |tag, name: &str| {
            ds.get_u16(tag)
                .ok_or_else(|| Error::Invalid(format!("{name} fehlt")))
        };
        let photometric = ds.get_str(t::PHOTOMETRIC).unwrap_or_default();
        let g = Geometry {
            rows: get(t::ROWS, "Rows")? as usize,
            cols: get(t::COLUMNS, "Columns")? as usize,
            bits: get(t::BITS_ALLOCATED, "BitsAllocated")? as usize,
            samples: ds.get_u16(t::SAMPLES_PER_PIXEL).unwrap_or(1) as usize,
            planar: ds.get_u16(t::PLANAR_CONFIGURATION).unwrap_or(0) == 1,
            mono1: photometric == "MONOCHROME1",
            ybr: photometric == "YBR_FULL",
        };
        if g.rows == 0 || g.cols == 0 {
            return Err(Error::Invalid("Bildgröße 0".into()));
        }
        match (photometric.as_str(), g.samples, g.bits) {
            ("MONOCHROME1" | "MONOCHROME2", 1, 8 | 16) | ("RGB" | "YBR_FULL", 3, 8) => Ok(g),
            (p, s, b) => Err(Error::Unsupported(format!(
                "Pixelformat {p}, {s} Samples, {b} Bit"
            ))),
        }
    }

    fn frame_bytes(&self) -> usize {
        self.rows * self.cols * self.samples * self.bits / 8
    }

    fn to_png(&self, frame: &[u8]) -> Result<Vec<u8>, Error> {
        let n = self.rows * self.cols;
        if self.samples == 1 {
            let mut gray: Vec<u8> = if self.bits == 8 {
                frame.to_vec()
            } else {
                // 16 Bit → 8 Bit über Min/Max-Fenster
                let v: Vec<u16> = frame
                    .chunks(2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .collect();
                let (lo, hi) = v
                    .iter()
                    .fold((u16::MAX, 0), |(l, h), &x| (l.min(x), h.max(x)));
                let span = (hi - lo).max(1) as u32;
                v.iter()
                    .map(|&x| (((x - lo) as u32 * 255) / span) as u8)
                    .collect()
            };
            if self.mono1 {
                gray.iter_mut().for_each(|p| *p = 255 - *p);
            }
            return Ok(png::encode(self.cols, self.rows, png::Color::Gray, &gray));
        }
        // 3 Samples → interleaved RGB
        let mut rgb = vec![0u8; n * 3];
        for i in 0..n {
            for c in 0..3 {
                rgb[i * 3 + c] = if self.planar {
                    frame[c * n + i]
                } else {
                    frame[i * 3 + c]
                };
            }
        }
        if self.ybr {
            for px in rgb.chunks_mut(3) {
                let (y, cb, cr) = (px[0] as f32, px[1] as f32 - 128.0, px[2] as f32 - 128.0);
                let clamp = |v: f32| v.round().clamp(0.0, 255.0) as u8;
                px[0] = clamp(y + 1.402 * cr);
                px[1] = clamp(y - 0.344136 * cb - 0.714136 * cr);
                px[2] = clamp(y + 1.772 * cb);
            }
        }
        Ok(png::encode(self.cols, self.rows, png::Color::Rgb, &rgb))
    }
}

#[cfg(test)]
mod tests;
