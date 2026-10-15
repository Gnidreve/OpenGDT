use super::*;

/// Standard für Einzelbild-Tests: Mehrbild-Instanzen als Einzelbilder (erstes Bild).
const STILLS: Options = Options {
    multi_frame: MultiFrame::FirstFrame,
};
use opengdt_dicom::dataset::{Element, Value};
use opengdt_dicom::Vr;

fn base(rows: u16, cols: u16) -> Dataset {
    let mut ds = Dataset::new();
    ds.set_u16(t::ROWS, rows);
    ds.set_u16(t::COLUMNS, cols);
    ds.set_u16(t::BITS_ALLOCATED, 8);
    ds.set_u16(t::SAMPLES_PER_PIXEL, 1);
    ds.set_str(t::PHOTOMETRIC, "MONOCHROME2");
    ds
}

#[test]
fn native_gray_to_png() {
    let mut ds = base(2, 3);
    ds.set_bytes(t::PIXEL_DATA, Vr::OW, vec![0, 50, 100, 150, 200, 250]);
    let out = convert_dataset(
        "1.2.840.10008.5.1.4.1.1.6.1",
        uid::EXPLICIT_VR_LE,
        &ds,
        STILLS,
    )
    .unwrap();
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].extension, "png");
    assert_eq!(&out[0].bytes[..8], b"\x89PNG\r\n\x1a\n");
    // Dump für externe Validierung (siehe Test-Skript im Scratchpad)
    if let Ok(dir) = std::env::var("OPENGDT_DUMP") {
        std::fs::write(format!("{dir}/gray.png"), &out[0].bytes).unwrap();
    }
}

#[test]
fn native_rgb_planar_and_ybr() {
    let mut ds = base(1, 2);
    ds.set_u16(t::SAMPLES_PER_PIXEL, 3);
    ds.set_str(t::PHOTOMETRIC, "RGB");
    ds.set_u16(t::PLANAR_CONFIGURATION, 1);
    // R-Plane, G-Plane, B-Plane (je 2 Pixel)
    ds.set_bytes(t::PIXEL_DATA, Vr::OW, vec![255, 0, 0, 255, 0, 0]);
    let out = convert_dataset("x", uid::EXPLICIT_VR_LE, &ds, STILLS).unwrap();
    if let Ok(dir) = std::env::var("OPENGDT_DUMP") {
        std::fs::write(format!("{dir}/rgb.png"), &out[0].bytes).unwrap();
    }
    assert_eq!(out[0].extension, "png");
}

#[test]
fn sixteen_bit_and_monochrome1() {
    let mut ds = base(1, 2);
    ds.set_u16(t::BITS_ALLOCATED, 16);
    ds.set_str(t::PHOTOMETRIC, "MONOCHROME1");
    ds.set_bytes(t::PIXEL_DATA, Vr::OW, vec![0, 0, 0xFF, 0x0F]);
    assert!(convert_dataset("x", uid::EXPLICIT_VR_LE, &ds, STILLS).is_ok());
}

#[test]
fn short_pixel_data_is_error() {
    let mut ds = base(10, 10);
    ds.set_bytes(t::PIXEL_DATA, Vr::OW, vec![0; 5]);
    assert!(matches!(
        convert_dataset("x", uid::EXPLICIT_VR_LE, &ds, STILLS),
        Err(Error::Invalid(_))
    ));
}

fn encapsulated(frags: Vec<Vec<u8>>) -> Dataset {
    let mut ds = Dataset::new();
    ds.insert(Element {
        tag: t::PIXEL_DATA,
        vr: Vr::OB,
        value: Value::Fragments(frags),
    });
    ds
}

#[test]
fn jpeg_passthrough_single_frame_multiple_fragments() {
    let ds = encapsulated(vec![vec![], vec![0xFF, 0xD8, 1, 2], vec![3, 4, 0xFF, 0xD9]]);
    let out = convert_dataset("x", uid::JPEG_BASELINE, &ds, STILLS).unwrap();
    assert_eq!(out[0].extension, "jpg");
    assert_eq!(out[0].bytes, vec![0xFF, 0xD8, 1, 2, 3, 4, 0xFF, 0xD9]);
}

#[test]
fn jpeg_multiframe_first_or_all() {
    let mut ds = encapsulated(vec![
        vec![],
        vec![0xFF, 0xD8, 0xFF, 0xD9],
        vec![0xFF, 0xD8, 9, 9, 0xFF, 0xD9],
    ]);
    ds.set_str(t::NUMBER_OF_FRAMES, "2");
    assert_eq!(
        convert_dataset("x", uid::JPEG_BASELINE, &ds, STILLS)
            .unwrap()
            .len(),
        1
    );
    let all = convert_dataset(
        "x",
        uid::JPEG_BASELINE,
        &ds,
        Options {
            multi_frame: MultiFrame::AllFrames,
        },
    )
    .unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(all[1].bytes.len(), 6);
}

#[test]
fn jpeg_odd_length_padding_is_stripped() {
    let ds = encapsulated(vec![vec![], vec![0xFF, 0xD8, 5, 0xFF, 0xD9, 0]]);
    let out = convert_dataset("x", uid::JPEG_BASELINE, &ds, STILLS).unwrap();
    assert_eq!(out[0].bytes, vec![0xFF, 0xD8, 5, 0xFF, 0xD9]);
}

#[test]
fn mpeg4_is_concatenated() {
    let ds = encapsulated(vec![vec![], vec![1, 2], vec![3]]);
    let out = convert_dataset("x", uid::MPEG4_HP41, &ds, STILLS).unwrap();
    assert_eq!((out[0].extension, out[0].kind), ("mp4", Kind::Video));
    assert_eq!(out[0].bytes, vec![1, 2, 3]);
}

#[test]
fn unsupported_compression_is_reported() {
    let ds = encapsulated(vec![vec![], vec![1]]);
    let r = convert_dataset("x", "1.2.840.10008.1.2.5", &ds, STILLS);
    assert!(matches!(r, Err(Error::Unsupported(_))));
}

#[test]
fn encapsulated_pdf_is_extracted() {
    let mut ds = Dataset::new();
    ds.set_bytes(ENCAPSULATED_DOCUMENT, Vr::OB, b"%PDF-1.4\0".to_vec());
    let out =
        convert_dataset(ENCAPSULATED_PDF_SOP_CLASS, uid::EXPLICIT_VR_LE, &ds, STILLS).unwrap();
    assert_eq!(out[0].bytes, b"%PDF-1.4");
}

#[test]
fn no_pixel_data_is_unsupported() {
    assert!(matches!(
        convert_dataset("x", uid::EXPLICIT_VR_LE, &Dataset::new(), STILLS),
        Err(Error::Unsupported(_))
    ));
}

// ---- Pixelprüfung über einen Mini-Decoder für unsere „stored“-PNGs ----------

/// Liest Breite, Höhe, Farbtyp und rohe Pixel aus einem von `png::encode` erzeugten PNG.
fn png_pixels(png: &[u8]) -> (usize, usize, u8, Vec<u8>) {
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    let (mut w, mut h, mut ct, mut z) = (0, 0, 0, Vec::new());
    let mut p = 8;
    while p < png.len() {
        let len = u32::from_be_bytes(png[p..p + 4].try_into().unwrap()) as usize;
        let ty = &png[p + 4..p + 8];
        let body = &png[p + 8..p + 8 + len];
        match ty {
            b"IHDR" => {
                w = u32::from_be_bytes(body[0..4].try_into().unwrap()) as usize;
                h = u32::from_be_bytes(body[4..8].try_into().unwrap()) as usize;
                ct = body[9];
            }
            b"IDAT" => z.extend_from_slice(body),
            _ => {}
        }
        p += 12 + len;
    }
    // zlib: 2 Byte Header, stored-Blöcke, 4 Byte Adler32
    let mut raw = Vec::new();
    let mut q = 2;
    loop {
        let last = z[q] & 1 == 1;
        let n = u16::from_le_bytes([z[q + 1], z[q + 2]]) as usize;
        raw.extend_from_slice(&z[q + 5..q + 5 + n]);
        q += 5 + n;
        if last {
            break;
        }
    }
    let bpp = if ct == 2 { 3 } else { 1 };
    let pixels = raw
        .chunks(w * bpp + 1)
        .flat_map(|row| row[1..].to_vec())
        .collect();
    (w, h, ct, pixels)
}

fn one(ds: &Dataset) -> Output {
    convert_dataset("x", uid::EXPLICIT_VR_LE, ds, STILLS)
        .unwrap()
        .remove(0)
}

#[test]
fn gray8_pixels_are_exact() {
    let mut ds = base(2, 3);
    ds.set_bytes(t::PIXEL_DATA, Vr::OW, vec![0, 50, 100, 150, 200, 250]);
    let (w, h, ct, px) = png_pixels(&one(&ds).bytes);
    assert_eq!((w, h, ct), (3, 2, 0));
    assert_eq!(px, vec![0, 50, 100, 150, 200, 250]);
}

#[test]
fn monochrome1_is_inverted() {
    let mut ds = base(1, 3);
    ds.set_str(t::PHOTOMETRIC, "MONOCHROME1");
    ds.set_bytes(t::PIXEL_DATA, Vr::OW, vec![0, 100, 255, 0]);
    assert_eq!(png_pixels(&one(&ds).bytes).3, vec![255, 155, 0]);
}

#[test]
fn sixteen_bit_is_scaled_to_full_range() {
    let mut ds = base(1, 3);
    ds.set_u16(t::BITS_ALLOCATED, 16);
    // 1000, 2000, 3000 → 0, 127, 255
    let v: Vec<u8> = [1000u16, 2000, 3000]
        .iter()
        .flat_map(|x| x.to_le_bytes())
        .collect();
    ds.set_bytes(t::PIXEL_DATA, Vr::OW, v);
    assert_eq!(png_pixels(&one(&ds).bytes).3, vec![0, 127, 255]);
}

#[test]
fn sixteen_bit_constant_image_does_not_divide_by_zero() {
    let mut ds = base(1, 2);
    ds.set_u16(t::BITS_ALLOCATED, 16);
    ds.set_bytes(t::PIXEL_DATA, Vr::OW, vec![5, 0, 5, 0]);
    assert_eq!(png_pixels(&one(&ds).bytes).3, vec![0, 0]);
}

fn rgb_ds(photometric: &str, planar: u16, data: Vec<u8>) -> Dataset {
    let mut ds = base(1, 2);
    ds.set_u16(t::SAMPLES_PER_PIXEL, 3);
    ds.set_str(t::PHOTOMETRIC, photometric);
    ds.set_u16(t::PLANAR_CONFIGURATION, planar);
    ds.set_bytes(t::PIXEL_DATA, Vr::OW, data);
    ds
}

#[test]
fn rgb_interleaved_and_planar_give_same_pixels() {
    // Pixel 1 = (10,20,30), Pixel 2 = (40,50,60)
    let inter = one(&rgb_ds("RGB", 0, vec![10, 20, 30, 40, 50, 60]));
    let planar = one(&rgb_ds("RGB", 1, vec![10, 40, 20, 50, 30, 60]));
    let (_, _, ct, a) = png_pixels(&inter.bytes);
    assert_eq!(ct, 2);
    assert_eq!(a, vec![10, 20, 30, 40, 50, 60]);
    assert_eq!(png_pixels(&planar.bytes).3, a);
}

#[test]
fn ybr_full_is_converted_to_rgb() {
    // Neutral: Y=128, Cb=Cr=128 → grau. Rot-lastig: Y=76, Cb=85, Cr=255 → ca. (255,0,0)
    let out = one(&rgb_ds("YBR_FULL", 0, vec![128, 128, 128, 76, 85, 255]));
    let px = png_pixels(&out.bytes).3;
    assert_eq!(&px[..3], &[128, 128, 128]);
    assert!(px[3] >= 250 && px[4] <= 5 && px[5] <= 5, "{:?}", &px[3..]);
    // Extremwerte werden begrenzt statt überzulaufen
    let clamp = one(&rgb_ds("YBR_FULL", 0, vec![255, 0, 255, 0, 255, 0]));
    assert_eq!(png_pixels(&clamp.bytes).0, 2);
}

#[test]
fn multiframe_native_all_frames_vs_first() {
    let mut ds = base(1, 2);
    ds.set_str(t::NUMBER_OF_FRAMES, "2");
    ds.set_bytes(t::PIXEL_DATA, Vr::OW, vec![1, 2, 3, 4]);
    let first = convert_dataset("x", uid::EXPLICIT_VR_LE, &ds, STILLS).unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(png_pixels(&first[0].bytes).3, vec![1, 2]);
    let all = convert_dataset(
        "x",
        uid::EXPLICIT_VR_LE,
        &ds,
        Options {
            multi_frame: MultiFrame::AllFrames,
        },
    )
    .unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(png_pixels(&all[1].bytes).3, vec![3, 4]);
}

#[test]
fn native_multiframe_with_missing_data_is_invalid() {
    let mut ds = base(1, 2);
    ds.set_str(t::NUMBER_OF_FRAMES, "3");
    ds.set_bytes(t::PIXEL_DATA, Vr::OW, vec![1, 2, 3, 4]);
    let r = convert_dataset(
        "x",
        uid::EXPLICIT_VR_LE,
        &ds,
        Options {
            multi_frame: MultiFrame::AllFrames,
        },
    );
    assert!(matches!(r, Err(Error::Invalid(_))), "{r:?}");
}

#[test]
fn bad_geometry_is_rejected() {
    let mut zero = base(0, 5);
    zero.set_bytes(t::PIXEL_DATA, Vr::OW, vec![]);
    assert!(matches!(
        convert_dataset("x", uid::EXPLICIT_VR_LE, &zero, STILLS),
        Err(Error::Invalid(_))
    ));

    let mut no_rows = Dataset::new();
    no_rows.set_bytes(t::PIXEL_DATA, Vr::OW, vec![0; 4]);
    assert!(matches!(
        convert_dataset("x", uid::EXPLICIT_VR_LE, &no_rows, STILLS),
        Err(Error::Invalid(_))
    ));

    let mut palette = base(1, 1);
    palette.set_str(t::PHOTOMETRIC, "PALETTE COLOR");
    palette.set_bytes(t::PIXEL_DATA, Vr::OW, vec![0, 0]);
    assert!(matches!(
        convert_dataset("x", uid::EXPLICIT_VR_LE, &palette, STILLS),
        Err(Error::Unsupported(_))
    ));

    let mut rgb16 = rgb_ds("RGB", 0, vec![0; 12]);
    rgb16.set_u16(t::BITS_ALLOCATED, 16);
    assert!(matches!(
        convert_dataset("x", uid::EXPLICIT_VR_LE, &rgb16, STILLS),
        Err(Error::Unsupported(_))
    ));
}

#[test]
fn pixel_data_as_sequence_is_invalid() {
    let mut ds = Dataset::new();
    ds.set_seq(t::PIXEL_DATA, vec![Dataset::new()]);
    assert!(matches!(
        convert_dataset("x", uid::EXPLICIT_VR_LE, &ds, STILLS),
        Err(Error::Invalid(_))
    ));
}

#[test]
fn pdf_without_document_is_invalid() {
    let r = convert_dataset(
        ENCAPSULATED_PDF_SOP_CLASS,
        uid::EXPLICIT_VR_LE,
        &Dataset::new(),
        STILLS,
    );
    assert!(matches!(r, Err(Error::Invalid(_))));
}

// ---- Frame-Zuordnung ----------------------------------------------------------

fn jpeg(tag: u8, len: usize) -> Vec<u8> {
    let mut v = vec![0xFF, 0xD8];
    v.extend(std::iter::repeat_n(tag, len));
    v.extend([0xFF, 0xD9]);
    v
}

fn offsets(list: &[u32]) -> Vec<u8> {
    list.iter().flat_map(|o| o.to_le_bytes()).collect()
}

fn multiframe(frames: &str, frags: Vec<Vec<u8>>) -> Dataset {
    let mut ds = encapsulated(frags);
    ds.set_str(t::NUMBER_OF_FRAMES, frames);
    ds
}

#[test]
fn offset_table_groups_fragments_into_frames() {
    // Frame 1 = zwei Fragmente (je 4+2 Bytes Nutzlast → 8+6 = 14 je Fragment), Frame 2 = ein Fragment.
    let (a1, a2, b) = (jpeg(1, 2), jpeg(2, 2), jpeg(3, 5));
    let second_start = (8 + a1.len() + 8 + a2.len()) as u32;
    let ds = multiframe(
        "2",
        vec![
            offsets(&[0, second_start]),
            a1.clone(),
            a2.clone(),
            b.clone(),
        ],
    );
    let out = convert_dataset(
        "x",
        uid::JPEG_BASELINE,
        &ds,
        Options {
            multi_frame: MultiFrame::AllFrames,
        },
    )
    .unwrap();
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].bytes, [a1, a2].concat());
    assert_eq!(out[1].bytes, b);
}

#[test]
fn offset_table_not_matching_fragments_is_invalid() {
    let ds = multiframe(
        "2",
        vec![offsets(&[0, 3]), jpeg(1, 2), jpeg(2, 2), jpeg(3, 2)],
    );
    let r = convert_dataset(
        "x",
        uid::JPEG_BASELINE,
        &ds,
        Options {
            multi_frame: MultiFrame::AllFrames,
        },
    );
    assert!(matches!(r, Err(Error::Invalid(_))), "{r:?}");
}

#[test]
fn ambiguous_fragment_assignment_is_unsupported() {
    // 2 Frames, 3 Fragmente, leere Offset-Tabelle → nicht eindeutig, nicht raten.
    let ds = multiframe("2", vec![vec![], jpeg(1, 2), jpeg(2, 2), jpeg(3, 2)]);
    let r = convert_dataset(
        "x",
        uid::JPEG_BASELINE,
        &ds,
        Options {
            multi_frame: MultiFrame::AllFrames,
        },
    );
    assert!(matches!(r, Err(Error::Unsupported(_))), "{r:?}");
}

#[test]
fn encapsulated_without_fragments_is_invalid() {
    let ds = encapsulated(vec![vec![]]);
    assert!(matches!(
        convert_dataset("x", uid::JPEG_BASELINE, &ds, STILLS),
        Err(Error::Invalid(_))
    ));
}

#[test]
fn one_fragment_per_frame_without_table() {
    let ds = multiframe("3", vec![vec![], jpeg(1, 1), jpeg(2, 1), jpeg(3, 1)]);
    let out = convert_dataset(
        "x",
        uid::JPEG_BASELINE,
        &ds,
        Options {
            multi_frame: MultiFrame::AllFrames,
        },
    )
    .unwrap();
    assert_eq!(out.len(), 3);
}

// ---- Einstieg über rohe Bytes + Fehlertexte --------------------------------------

#[test]
fn convert_from_raw_bytes_explicit_and_implicit() {
    let mut ds = base(1, 2);
    ds.set_bytes(t::PIXEL_DATA, Vr::OW, vec![9, 8]);
    for (ts, explicit) in [(uid::EXPLICIT_VR_LE, true), (uid::IMPLICIT_VR_LE, false)] {
        let out = convert("x", ts, &ds.encode(explicit), STILLS).unwrap();
        assert_eq!(png_pixels(&out[0].bytes).3, vec![9, 8]);
    }
}

#[test]
fn convert_rejects_bad_transfer_syntax_and_garbage() {
    assert!(matches!(
        convert("x", uid::EXPLICIT_VR_BE, &[], STILLS),
        Err(Error::Unsupported(_))
    ));
    assert!(matches!(
        convert(
            "x",
            uid::EXPLICIT_VR_LE,
            &[0x28, 0x00, 0x10, 0x00, b'U', b'S', 0xFF],
            STILLS
        ),
        Err(Error::Invalid(_))
    ));
}

#[test]
fn error_messages_are_readable() {
    assert!(Error::Unsupported("x".into())
        .to_string()
        .contains("nicht unterstützt"));
    assert!(Error::Invalid("y".into()).to_string().contains("ungültig"));
}

// ---- Cine-Clips als AVI ------------------------------------------------------------

use crate::avi::tests::walk;

const VIDEO: Options = Options {
    multi_frame: MultiFrame::Video,
};

/// Minimales „JPEG“ mit SOF0 (Breite × Höhe) – reicht für Einpacken und Größenermittlung.
fn jpeg_sof(w: u16, h: u16, tag: u8) -> Vec<u8> {
    let mut v = vec![0xFF, 0xD8, 0xFF, 0xC0, 0x00, 0x0B, 0x08];
    v.extend(h.to_be_bytes());
    v.extend(w.to_be_bytes());
    v.extend([1, 1, 0x11, 0, tag, tag, 0xFF, 0xD9]);
    v
}

/// Nutzdaten der Frame-Chunks einer AVI-Datei.
fn avi_frames(avi: &[u8]) -> Vec<Vec<u8>> {
    walk(avi)
        .0
        .into_iter()
        .map(|(p, n)| avi[p + 8..p + 8 + n].to_vec())
        .collect()
}

fn avi_u32(avi: &[u8], tag: &[u8; 4], off: usize) -> u32 {
    let p = avi.windows(4).position(|w| w == tag).unwrap() + 8 + off;
    u32::from_le_bytes(avi[p..p + 4].try_into().unwrap())
}

#[test]
fn jpeg_cine_becomes_mjpeg_avi_by_default() {
    let frames = [
        jpeg_sof(96, 64, 1),
        jpeg_sof(96, 64, 2),
        jpeg_sof(96, 64, 3),
    ];
    let mut ds = multiframe(
        "3",
        std::iter::once(vec![])
            .chain(frames.iter().cloned())
            .collect(),
    );
    ds.set_str(t::FRAME_TIME, "50");
    let out = convert_dataset("x", uid::JPEG_BASELINE, &ds, Options::default()).unwrap();
    assert_eq!(out.len(), 1);
    let o = &out[0];
    assert_eq!(
        (o.kind, o.extension, o.gdt_format),
        (Kind::Video, "avi", "AVI")
    );
    assert_eq!(
        avi_frames(&o.bytes),
        frames.to_vec(),
        "JPEG-Frames unverändert eingepackt"
    );
    assert_eq!(avi_u32(&o.bytes, b"avih", 0), 50_000, "50 ms je Bild");
    assert_eq!(avi_u32(&o.bytes, b"avih", 16), 3);
    assert_eq!(
        (
            avi_u32(&o.bytes, b"avih", 32),
            avi_u32(&o.bytes, b"avih", 36)
        ),
        (96, 64)
    );
}

#[test]
fn jpeg_cine_policies() {
    let frames = vec![vec![], jpeg_sof(8, 8, 1), jpeg_sof(8, 8, 2)];
    let ds = multiframe("2", frames);
    let first = convert_dataset("x", uid::JPEG_BASELINE, &ds, STILLS).unwrap();
    assert_eq!((first.len(), first[0].extension), (1, "jpg"));
    let all = convert_dataset(
        "x",
        uid::JPEG_BASELINE,
        &ds,
        Options {
            multi_frame: MultiFrame::AllFrames,
        },
    )
    .unwrap();
    assert_eq!((all.len(), all[0].extension), (2, "jpg"));
    // Ein einzelnes Bild bleibt ein JPEG, auch im Video-Modus
    let single = encapsulated(vec![vec![], jpeg_sof(8, 8, 1)]);
    let one = convert_dataset("x", uid::JPEG_BASELINE, &single, VIDEO).unwrap();
    assert_eq!((one.len(), one[0].extension), (1, "jpg"));
}

#[test]
fn jpeg_cine_padding_is_removed_before_packing() {
    let mut odd = jpeg_sof(8, 8, 1);
    odd.push(0); // DICOM-Füllbyte hinter EOI
    let ds = multiframe("2", vec![vec![], odd, jpeg_sof(8, 8, 2)]);
    let out = convert_dataset("x", uid::JPEG_BASELINE, &ds, VIDEO).unwrap();
    assert_eq!(avi_frames(&out[0].bytes)[0], jpeg_sof(8, 8, 1));
}

#[test]
fn jpeg_cine_size_comes_from_sof_with_dataset_fallback() {
    // Keine SOF-Marker: Größe aus Rows/Columns des Datensatzes
    let mut ds = multiframe(
        "2",
        vec![
            vec![],
            vec![0xFF, 0xD8, 1, 0xFF, 0xD9],
            vec![0xFF, 0xD8, 2, 0xFF, 0xD9],
        ],
    );
    ds.set_u16(t::ROWS, 30);
    ds.set_u16(t::COLUMNS, 40);
    let out = convert_dataset("x", uid::JPEG_BASELINE, &ds, VIDEO).unwrap();
    assert_eq!(
        (
            avi_u32(&out[0].bytes, b"avih", 32),
            avi_u32(&out[0].bytes, b"avih", 36)
        ),
        (40, 30)
    );
    // Weder SOF noch Rows/Columns: Fehler statt Raten
    let none = multiframe(
        "2",
        vec![
            vec![],
            vec![0xFF, 0xD8, 1, 0xFF, 0xD9],
            vec![0xFF, 0xD8, 2, 0xFF, 0xD9],
        ],
    );
    assert!(matches!(
        convert_dataset("x", uid::JPEG_BASELINE, &none, VIDEO),
        Err(Error::Invalid(_))
    ));
}

#[test]
fn jpeg_cine_with_changing_frame_size_is_invalid() {
    let ds = multiframe("2", vec![vec![], jpeg_sof(8, 8, 1), jpeg_sof(16, 8, 2)]);
    assert!(matches!(
        convert_dataset("x", uid::JPEG_BASELINE, &ds, VIDEO),
        Err(Error::Invalid(_))
    ));
}

#[test]
fn frame_rate_sources_and_fallbacks() {
    let us = |f: &dyn Fn(&mut Dataset)| {
        let mut d = Dataset::new();
        f(&mut d);
        frame_interval_us(&d)
    };
    assert_eq!(us(&|d| d.set_str(t::FRAME_TIME, "33.333")), 33_333);
    assert_eq!(
        us(&|d| d.set_str(t::FRAME_TIME, "40\\40\\41")),
        40_000,
        "Vektor: erster Wert"
    );
    assert_eq!(us(&|d| d.set_str(t::CINE_RATE, "20")), 50_000);
    assert_eq!(us(&|d| d.set_str(Tag(0x0008, 0x2144), "10")), 100_000);
    assert_eq!(
        us(&|d| {
            d.set_str(t::FRAME_TIME, "20");
            d.set_str(t::CINE_RATE, "10");
        }),
        20_000,
        "Frame Time hat Vorrang"
    );
    assert_eq!(us(&|_| {}), 40_000, "Standard 25 fps");
    for bad in ["0", "-5", "abc", "100000", "0.0001"] {
        assert_eq!(us(&|d| d.set_str(t::FRAME_TIME, bad)), 40_000, "{bad:?}");
    }
    assert_eq!(us(&|d| d.set_str(t::CINE_RATE, "0")), 40_000);
}

#[test]
fn native_gray_cine_becomes_uncompressed_avi_with_palette() {
    let mut ds = base(2, 3);
    ds.set_str(t::NUMBER_OF_FRAMES, "2");
    ds.set_bytes(
        t::PIXEL_DATA,
        Vr::OW,
        vec![0, 50, 100, 150, 200, 250, 1, 2, 3, 4, 5, 6],
    );
    let out = convert_dataset("x", uid::EXPLICIT_VR_LE, &ds, VIDEO).unwrap();
    let (kind, ext) = (out[0].kind, out[0].extension);
    assert_eq!((kind, ext), (Kind::Video, "avi"));
    let f = avi_frames(&out[0].bytes);
    assert_eq!(f.len(), 2);
    // bottom-up, Zeilen auf 4 Byte gepolstert
    assert_eq!(f[0], vec![150, 200, 250, 0, 0, 50, 100, 0]);
    assert_eq!(f[1], vec![4, 5, 6, 0, 1, 2, 3, 0]);
    assert!(
        out[0].bytes.windows(4).any(|w| w == [255, 255, 255, 0]),
        "Graupalette enthalten"
    );
    if let Ok(dir) = std::env::var("OPENGDT_DUMP") {
        std::fs::write(format!("{dir}/gray.avi"), &out[0].bytes).unwrap();
    }
}

#[test]
fn native_rgb_cine_is_bgr24_avi() {
    let mut ds = rgb_ds(
        "RGB",
        0,
        vec![10, 20, 30, 40, 50, 60, 11, 21, 31, 41, 51, 61],
    );
    ds.set_str(t::NUMBER_OF_FRAMES, "2");
    let out = convert_dataset("x", uid::EXPLICIT_VR_LE, &ds, VIDEO).unwrap();
    let f = avi_frames(&out[0].bytes);
    // 1 Zeile × 2 Pixel: B,G,R je Pixel, Stride 8 → 6 Bytes + 2 Füllbytes
    assert_eq!(f[0], vec![30, 20, 10, 60, 50, 40, 0, 0]);
    assert_eq!(f[1], vec![31, 21, 11, 61, 51, 41, 0, 0]);
}

#[test]
fn sixteen_bit_cine_uses_one_window_for_all_frames() {
    let mut ds = base(1, 2);
    ds.set_u16(t::BITS_ALLOCATED, 16);
    ds.set_str(t::NUMBER_OF_FRAMES, "2");
    let v: Vec<u8> = [0u16, 1000, 1000, 2000]
        .iter()
        .flat_map(|x| x.to_le_bytes())
        .collect();
    ds.set_bytes(t::PIXEL_DATA, Vr::OW, v);
    let out = convert_dataset("x", uid::EXPLICIT_VR_LE, &ds, VIDEO).unwrap();
    let f = avi_frames(&out[0].bytes);
    // Fenster 0..2000 für beide Frames (je Frame skaliert wäre 0/255 und 0/255)
    assert_eq!(&f[0][..2], &[0, 127]);
    assert_eq!(&f[1][..2], &[127, 255]);
}

#[test]
fn native_cine_with_missing_data_is_invalid() {
    let mut ds = base(1, 2);
    ds.set_str(t::NUMBER_OF_FRAMES, "3");
    ds.set_bytes(t::PIXEL_DATA, Vr::OW, vec![1, 2, 3, 4]);
    assert!(matches!(
        convert_dataset("x", uid::EXPLICIT_VR_LE, &ds, VIDEO),
        Err(Error::Invalid(_))
    ));
}

#[test]
fn native_cine_policies() {
    let mut ds = base(1, 2);
    ds.set_str(t::NUMBER_OF_FRAMES, "2");
    ds.set_bytes(t::PIXEL_DATA, Vr::OW, vec![1, 2, 3, 4]);
    let first = convert_dataset("x", uid::EXPLICIT_VR_LE, &ds, STILLS).unwrap();
    assert_eq!((first.len(), first[0].extension), (1, "png"));
    // Ein Einzelbild bleibt PNG, auch im Video-Modus
    let mut single = base(1, 2);
    single.set_bytes(t::PIXEL_DATA, Vr::OW, vec![1, 2]);
    assert_eq!(
        convert_dataset("x", uid::EXPLICIT_VR_LE, &single, VIDEO).unwrap()[0].extension,
        "png"
    );
}

// ---- Fuzz-Rauchtest: beliebige Geometrie/Datenlängen dürfen nie einen Panic auslösen ----------

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn pick<T: Copy>(&mut self, v: &[T]) -> T {
        v[(self.next() % v.len() as u64) as usize]
    }
}

#[test]
fn random_geometry_and_data_never_panic() {
    let mut rng = Rng(0xDEAD_BEEF_CAFE_F00D);
    let dims = [0u16, 1, 2, 3, 7, 64, 255, 1000, 65535];
    let phot = [
        "MONOCHROME1",
        "MONOCHROME2",
        "RGB",
        "YBR_FULL",
        "YBR_FULL_422",
        "PALETTE COLOR",
        "",
    ];
    let frames = ["", "0", "1", "2", "5", "-3", "abc", "100000", "4294967296"];
    let policies = [
        MultiFrame::Video,
        MultiFrame::FirstFrame,
        MultiFrame::AllFrames,
    ];
    let mut panics = Vec::new();
    for i in 0..30_000 {
        let mut ds = Dataset::new();
        ds.set_u16(t::ROWS, rng.pick(&dims));
        ds.set_u16(t::COLUMNS, rng.pick(&dims));
        ds.set_u16(t::BITS_ALLOCATED, rng.pick(&[1, 8, 12, 16, 32, 0]));
        ds.set_u16(t::SAMPLES_PER_PIXEL, rng.pick(&[0, 1, 3, 4]));
        ds.set_u16(t::PLANAR_CONFIGURATION, rng.pick(&[0, 1, 2]));
        ds.set_str(t::PHOTOMETRIC, rng.pick(&phot));
        ds.set_str(t::NUMBER_OF_FRAMES, rng.pick(&frames));
        let len = rng.pick(&[0usize, 1, 2, 3, 5, 12, 100, 4096, 70000]);
        let data: Vec<u8> = (0..len).map(|_| rng.next() as u8).collect();
        ds.set_bytes(t::PIXEL_DATA, Vr::OW, data);
        let policy = rng.pick(&policies);
        let r = std::panic::catch_unwind(|| {
            let _ = convert_dataset(
                "x",
                uid::EXPLICIT_VR_LE,
                &ds,
                Options {
                    multi_frame: policy,
                },
            );
        });
        if r.is_err() {
            panics.push(i);
            if panics.len() >= 3 {
                break;
            }
        }
    }
    assert!(panics.is_empty(), "Panic bei Durchläufen {panics:?}");
}

#[test]
fn random_encapsulated_data_never_panics() {
    let mut rng = Rng(0x1234_5678_9ABC_DEF1);
    let tss = [
        uid::JPEG_BASELINE,
        uid::MPEG4_HP41,
        "1.2.840.10008.1.2.5",
        uid::EXPLICIT_VR_LE,
    ];
    for _ in 0..20_000 {
        let nfrags = (rng.next() % 6) as usize;
        let frags: Vec<Vec<u8>> = (0..nfrags)
            .map(|_| {
                let n = (rng.next() % 40) as usize;
                (0..n).map(|_| rng.next() as u8).collect()
            })
            .collect();
        let mut ds = encapsulated(frags);
        ds.set_str(
            t::NUMBER_OF_FRAMES,
            ["", "1", "2", "3", "7", "0"][(rng.next() % 6) as usize],
        );
        let (ts, policy) = (
            tss[(rng.next() % 4) as usize],
            [
                MultiFrame::Video,
                MultiFrame::AllFrames,
                MultiFrame::FirstFrame,
            ][(rng.next() % 3) as usize],
        );
        let r = std::panic::catch_unwind(|| {
            let _ = convert_dataset(
                "x",
                ts,
                &ds,
                Options {
                    multi_frame: policy,
                },
            );
        });
        assert!(r.is_ok(), "Panic bei ts={ts} policy={policy:?}");
    }
}
