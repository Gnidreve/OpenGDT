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
