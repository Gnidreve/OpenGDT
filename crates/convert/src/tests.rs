use super::*;
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
        Options::default(),
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
    let out = convert_dataset("x", uid::EXPLICIT_VR_LE, &ds, Options::default()).unwrap();
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
    assert!(convert_dataset("x", uid::EXPLICIT_VR_LE, &ds, Options::default()).is_ok());
}

#[test]
fn short_pixel_data_is_error() {
    let mut ds = base(10, 10);
    ds.set_bytes(t::PIXEL_DATA, Vr::OW, vec![0; 5]);
    assert!(matches!(
        convert_dataset("x", uid::EXPLICIT_VR_LE, &ds, Options::default()),
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
    let out = convert_dataset("x", uid::JPEG_BASELINE, &ds, Options::default()).unwrap();
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
        convert_dataset("x", uid::JPEG_BASELINE, &ds, Options::default())
            .unwrap()
            .len(),
        1
    );
    let all = convert_dataset("x", uid::JPEG_BASELINE, &ds, Options { all_frames: true }).unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(all[1].bytes.len(), 6);
}

#[test]
fn jpeg_odd_length_padding_is_stripped() {
    let ds = encapsulated(vec![vec![], vec![0xFF, 0xD8, 5, 0xFF, 0xD9, 0]]);
    let out = convert_dataset("x", uid::JPEG_BASELINE, &ds, Options::default()).unwrap();
    assert_eq!(out[0].bytes, vec![0xFF, 0xD8, 5, 0xFF, 0xD9]);
}

#[test]
fn mpeg4_is_concatenated() {
    let ds = encapsulated(vec![vec![], vec![1, 2], vec![3]]);
    let out = convert_dataset("x", uid::MPEG4_HP41, &ds, Options::default()).unwrap();
    assert_eq!((out[0].extension, out[0].kind), ("mp4", Kind::Video));
    assert_eq!(out[0].bytes, vec![1, 2, 3]);
}

#[test]
fn unsupported_compression_is_reported() {
    let ds = encapsulated(vec![vec![], vec![1]]);
    let r = convert_dataset("x", "1.2.840.10008.1.2.5", &ds, Options::default());
    assert!(matches!(r, Err(Error::Unsupported(_))));
}

#[test]
fn encapsulated_pdf_is_extracted() {
    let mut ds = Dataset::new();
    ds.set_bytes(ENCAPSULATED_DOCUMENT, Vr::OB, b"%PDF-1.4\0".to_vec());
    let out = convert_dataset(
        ENCAPSULATED_PDF_SOP_CLASS,
        uid::EXPLICIT_VR_LE,
        &ds,
        Options::default(),
    )
    .unwrap();
    assert_eq!(out[0].bytes, b"%PDF-1.4");
}

#[test]
fn no_pixel_data_is_unsupported() {
    assert!(matches!(
        convert_dataset(
            "x",
            uid::EXPLICIT_VR_LE,
            &Dataset::new(),
            Options::default()
        ),
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
    convert_dataset("x", uid::EXPLICIT_VR_LE, ds, Options::default())
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
    let first = convert_dataset("x", uid::EXPLICIT_VR_LE, &ds, Options::default()).unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(png_pixels(&first[0].bytes).3, vec![1, 2]);
    let all = convert_dataset("x", uid::EXPLICIT_VR_LE, &ds, Options { all_frames: true }).unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(png_pixels(&all[1].bytes).3, vec![3, 4]);
}

#[test]
fn native_multiframe_with_missing_data_is_invalid() {
    let mut ds = base(1, 2);
    ds.set_str(t::NUMBER_OF_FRAMES, "3");
    ds.set_bytes(t::PIXEL_DATA, Vr::OW, vec![1, 2, 3, 4]);
    let r = convert_dataset("x", uid::EXPLICIT_VR_LE, &ds, Options { all_frames: true });
    assert!(matches!(r, Err(Error::Invalid(_))), "{r:?}");
}

#[test]
fn bad_geometry_is_rejected() {
    let mut zero = base(0, 5);
    zero.set_bytes(t::PIXEL_DATA, Vr::OW, vec![]);
    assert!(matches!(
        convert_dataset("x", uid::EXPLICIT_VR_LE, &zero, Options::default()),
        Err(Error::Invalid(_))
    ));

    let mut no_rows = Dataset::new();
    no_rows.set_bytes(t::PIXEL_DATA, Vr::OW, vec![0; 4]);
    assert!(matches!(
        convert_dataset("x", uid::EXPLICIT_VR_LE, &no_rows, Options::default()),
        Err(Error::Invalid(_))
    ));

    let mut palette = base(1, 1);
    palette.set_str(t::PHOTOMETRIC, "PALETTE COLOR");
    palette.set_bytes(t::PIXEL_DATA, Vr::OW, vec![0, 0]);
    assert!(matches!(
        convert_dataset("x", uid::EXPLICIT_VR_LE, &palette, Options::default()),
        Err(Error::Unsupported(_))
    ));

    let mut rgb16 = rgb_ds("RGB", 0, vec![0; 12]);
    rgb16.set_u16(t::BITS_ALLOCATED, 16);
    assert!(matches!(
        convert_dataset("x", uid::EXPLICIT_VR_LE, &rgb16, Options::default()),
        Err(Error::Unsupported(_))
    ));
}

#[test]
fn pixel_data_as_sequence_is_invalid() {
    let mut ds = Dataset::new();
    ds.set_seq(t::PIXEL_DATA, vec![Dataset::new()]);
    assert!(matches!(
        convert_dataset("x", uid::EXPLICIT_VR_LE, &ds, Options::default()),
        Err(Error::Invalid(_))
    ));
}

#[test]
fn pdf_without_document_is_invalid() {
    let r = convert_dataset(
        ENCAPSULATED_PDF_SOP_CLASS,
        uid::EXPLICIT_VR_LE,
        &Dataset::new(),
        Options::default(),
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
    let out = convert_dataset("x", uid::JPEG_BASELINE, &ds, Options { all_frames: true }).unwrap();
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
    let r = convert_dataset("x", uid::JPEG_BASELINE, &ds, Options { all_frames: true });
    assert!(matches!(r, Err(Error::Invalid(_))), "{r:?}");
}

#[test]
fn ambiguous_fragment_assignment_is_unsupported() {
    // 2 Frames, 3 Fragmente, leere Offset-Tabelle → nicht eindeutig, nicht raten.
    let ds = multiframe("2", vec![vec![], jpeg(1, 2), jpeg(2, 2), jpeg(3, 2)]);
    let r = convert_dataset("x", uid::JPEG_BASELINE, &ds, Options { all_frames: true });
    assert!(matches!(r, Err(Error::Unsupported(_))), "{r:?}");
}

#[test]
fn encapsulated_without_fragments_is_invalid() {
    let ds = encapsulated(vec![vec![]]);
    assert!(matches!(
        convert_dataset("x", uid::JPEG_BASELINE, &ds, Options::default()),
        Err(Error::Invalid(_))
    ));
}

#[test]
fn one_fragment_per_frame_without_table() {
    let ds = multiframe("3", vec![vec![], jpeg(1, 1), jpeg(2, 1), jpeg(3, 1)]);
    let out = convert_dataset("x", uid::JPEG_BASELINE, &ds, Options { all_frames: true }).unwrap();
    assert_eq!(out.len(), 3);
}

// ---- Einstieg über rohe Bytes + Fehlertexte --------------------------------------

#[test]
fn convert_from_raw_bytes_explicit_and_implicit() {
    let mut ds = base(1, 2);
    ds.set_bytes(t::PIXEL_DATA, Vr::OW, vec![9, 8]);
    for (ts, explicit) in [(uid::EXPLICIT_VR_LE, true), (uid::IMPLICIT_VR_LE, false)] {
        let out = convert("x", ts, &ds.encode(explicit), Options::default()).unwrap();
        assert_eq!(png_pixels(&out[0].bytes).3, vec![9, 8]);
    }
}

#[test]
fn convert_rejects_bad_transfer_syntax_and_garbage() {
    assert!(matches!(
        convert("x", uid::EXPLICIT_VR_BE, &[], Options::default()),
        Err(Error::Unsupported(_))
    ));
    assert!(matches!(
        convert(
            "x",
            uid::EXPLICIT_VR_LE,
            &[0x28, 0x00, 0x10, 0x00, b'U', b'S', 0xFF],
            Options::default()
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
