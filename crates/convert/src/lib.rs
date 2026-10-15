//! Konvertiert DICOM-Instanzen in Dateien, die ein PVS anzeigen kann.
//!
//! Prinzip: *nichts neu kodieren, was schon in einem Standardformat vorliegt.*
//! JPEG-Baseline-Frames und MPEG-4-Streams stecken in DICOM unverändert
//! in den Fragmenten und werden nur ausgepackt. Unkomprimierte Pixeldaten
//! werden als PNG geschrieben (eigener Encoder, `png.rs`).

mod avi;
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

/// Umgang mit Mehrbild-Instanzen (Cine-Clips).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MultiFrame {
    /// Ein Video: JPEG-Frames → MJPEG-AVI (verlustfrei eingepackt), unkomprimiert → unkomprimiertes AVI.
    #[default]
    Video,
    /// Nur der erste Frame als Einzelbild.
    FirstFrame,
    /// Jeder Frame als Einzelbild.
    AllFrames,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Options {
    pub multi_frame: MultiFrame,
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
        Value::Fragments(frags) => convert_encapsulated(transfer_syntax, ds, frags, frames, opts),
        Value::Bytes(raw) => {
            let geo = Geometry::from_dataset(ds)?;
            let fsize = geo.frame_bytes();
            let video = frames > 1 && opts.multi_frame == MultiFrame::Video;
            let count = match (frames, opts.multi_frame) {
                (1, _) | (_, MultiFrame::FirstFrame) => 1,
                _ => frames,
            };
            if raw.len() < fsize * count {
                return Err(Error::Invalid("Pixeldaten kürzer als Bildgeometrie".into()));
            }
            let frame = |i: usize| &raw[i * fsize..(i + 1) * fsize];
            if video {
                let all: Vec<&[u8]> = (0..count).map(frame).collect();
                return geo.to_video(&all, frame_interval_us(ds)).map(|o| vec![o]);
            }
            (0..count)
                .map(|i| {
                    Ok(Output {
                        kind: Kind::Still,
                        extension: "png",
                        gdt_format: "PNG",
                        bytes: geo.to_png(frame(i))?,
                    })
                })
                .collect()
        }
        Value::Sequence(_) => Err(Error::Invalid("Pixeldaten als Sequenz".into())),
    }
}

fn convert_encapsulated(
    ts: &str,
    ds: &Dataset,
    frags: &[Vec<u8>],
    frames: usize,
    opts: Options,
) -> Result<Vec<Output>, Error> {
    // Erstes Fragment = Basic Offset Table.
    let data = frags.get(1..).unwrap_or(&[]);
    match ts {
        uid::JPEG_BASELINE => {
            let jpegs: Vec<Vec<u8>> = split_frames(frags, frames)?.into_iter().map(strip_pad).collect();
            if jpegs.len() > 1 && opts.multi_frame == MultiFrame::Video {
                return mjpeg_video(ds, &jpegs).map(|o| vec![o]);
            }
            let take = if opts.multi_frame == MultiFrame::AllFrames { jpegs.len() } else { 1 };
            Ok(jpegs
                .into_iter()
                .take(take)
                .map(|bytes| Output {
                    kind: Kind::Still,
                    extension: "jpg",
                    gdt_format: "JPG",
                    bytes,
                })
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

/// Bilddauer in µs: Frame Time (ms), sonst Cine Rate bzw. Recommended Display Frame Rate (Bilder/s), sonst 25 fps.
fn frame_interval_us(ds: &Dataset) -> u32 {
    let from_ms = ds
        .get_str(t::FRAME_TIME)
        .and_then(|s| {
            s.split('\\')
                .next()
                .and_then(|v| v.trim().parse::<f64>().ok())
        })
        .map(|ms| ms * 1000.0);
    let from_rate = |tag| {
        ds.get_str(tag)
            .and_then(|s| s.trim().parse::<f64>().ok())
            .filter(|r| *r > 0.0)
            .map(|fps| 1_000_000.0 / fps)
    };
    let us = from_ms
        .or_else(|| from_rate(t::CINE_RATE))
        .or_else(|| from_rate(Tag(0x0008, 0x2144)))
        .unwrap_or(40_000.0);
    // Unsinnige Angaben (0, negativ, > 1 s) → Standard
    if (1_000.0..=1_000_000.0).contains(&us) {
        us.round() as u32
    } else {
        40_000
    }
}

/// JPEG-Frames unverändert als MJPEG-AVI einpacken.
fn mjpeg_video(ds: &Dataset, jpegs: &[Vec<u8>]) -> Result<Output, Error> {
    let (w, h) = avi::jpeg_dimensions(&jpegs[0])
        .or_else(|| Some((ds.get_u16(t::COLUMNS)? as u32, ds.get_u16(t::ROWS)? as u32)))
        .ok_or_else(|| Error::Invalid("Bildgröße des JPEG nicht ermittelbar".into()))?;
    if let Some(bad) = jpegs
        .iter()
        .position(|j| avi::jpeg_dimensions(j).is_some_and(|d| d != (w, h)))
    {
        return Err(Error::Invalid(format!(
            "Frame {bad} hat eine andere Größe als Frame 0"
        )));
    }
    let bytes = avi::write(&avi::Video {
        width: w,
        height: h,
        frame_us: frame_interval_us(ds),
        codec: avi::Codec::Mjpeg,
        frames: jpegs,
    })?;
    Ok(Output {
        kind: Kind::Video,
        extension: "avi",
        gdt_format: "AVI",
        bytes,
    })
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

    /// Frames → 8 Bit, interleaved (Grau bzw. RGB). 16-Bit-Grau wird über das Min/Max-Fenster
    /// **aller übergebenen Frames** skaliert (kein Flackern in Clips).
    fn to_8bit(&self, frames: &[&[u8]]) -> Vec<Vec<u8>> {
        let n = self.rows * self.cols;
        if self.samples == 1 {
            let wide: Vec<Vec<u16>> = if self.bits == 16 {
                frames
                    .iter()
                    .map(|f| {
                        f.chunks(2)
                            .map(|c| u16::from_le_bytes([c[0], c[1]]))
                            .collect()
                    })
                    .collect()
            } else {
                Vec::new()
            };
            let (lo, hi) = wide
                .iter()
                .flatten()
                .fold((u16::MAX, 0), |(l, h), &x| (l.min(x), h.max(x)));
            let span = (hi.saturating_sub(lo)).max(1) as u32;
            return (0..frames.len())
                .map(|i| {
                    let mut gray: Vec<u8> = if self.bits == 8 {
                        frames[i].to_vec()
                    } else {
                        wide[i]
                            .iter()
                            .map(|&x| (((x - lo) as u32 * 255) / span) as u8)
                            .collect()
                    };
                    if self.mono1 {
                        gray.iter_mut().for_each(|p| *p = 255 - *p);
                    }
                    gray
                })
                .collect();
        }
        frames
            .iter()
            .map(|frame| {
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
                        let (y, cb, cr) =
                            (px[0] as f32, px[1] as f32 - 128.0, px[2] as f32 - 128.0);
                        let clamp = |v: f32| v.round().clamp(0.0, 255.0) as u8;
                        px[0] = clamp(y + 1.402 * cr);
                        px[1] = clamp(y - 0.344136 * cb - 0.714136 * cr);
                        px[2] = clamp(y + 1.772 * cb);
                    }
                }
                rgb
            })
            .collect()
    }

    fn to_png(&self, frame: &[u8]) -> Result<Vec<u8>, Error> {
        let px = self.to_8bit(&[frame]).remove(0);
        let color = if self.samples == 1 {
            png::Color::Gray
        } else {
            png::Color::Rgb
        };
        Ok(png::encode(self.cols, self.rows, color, &px))
    }

    /// Unkomprimiertes AVI (Graustufen: 8 Bit mit Graupalette; Farbe: 24 Bit BGR).
    fn to_video(&self, frames: &[&[u8]], frame_us: u32) -> Result<Output, Error> {
        let gray = self.samples == 1;
        let dib: Vec<Vec<u8>> = self
            .to_8bit(frames)
            .into_iter()
            .map(|mut px| {
                if !gray {
                    px.chunks_mut(3).for_each(|p| p.swap(0, 2)); // RGB → BGR
                }
                avi::dib_rows(self.cols, self.rows, if gray { 1 } else { 3 }, &px)
            })
            .collect();
        let bytes = avi::write(&avi::Video {
            width: self.cols as u32,
            height: self.rows as u32,
            frame_us,
            codec: if gray {
                avi::Codec::Gray8
            } else {
                avi::Codec::Bgr24
            },
            frames: &dib,
        })?;
        Ok(Output {
            kind: Kind::Video,
            extension: "avi",
            gdt_format: "AVI",
            bytes,
        })
    }
}

#[cfg(test)]
mod tests;
