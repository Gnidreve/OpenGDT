//! Verhalten des Mindray DP-70/DP-50 laut DICOM Conformance Statement
//! (P/N 046-001907-00, V1.0, 13.01.2011) gegen die komplette Bridge.
//! Quelle der Daten: `devices/mindray-dp/dicom-conformance.json`.

use opengdt_bridge::config::Config;
use opengdt_dicom::scu::{connect, ScuConfig};
use opengdt_dicom::{uid, Dataset, Tag, Vr};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn tmp() -> PathBuf {
    let d = std::env::temp_dir().join(format!("opengdt-dp-{}", uid::generate()));
    fs::create_dir_all(&d).unwrap();
    d
}

fn conf(root: &std::path::Path, extra: &str) -> Config {
    Config::parse(&format!(
        "listen = 127.0.0.1:0\ngdt_import_dir = {r}/in\ngdt_export_dir = {r}/out\nworklist_dir = {r}/wl\n\
         archive_dir = {r}/arch\nexport_dir = {r}/export\n{extra}\n",
        r = root.display()
    ))
    .unwrap()
}

fn scu() -> ScuConfig {
    ScuConfig {
        calling_ae: "DP50".into(),
        called_ae: "OPENGDT".into(),
        timeout: Duration::from_secs(5),
    }
}

fn gdt_request() -> Vec<u8> {
    let mut v = Vec::new();
    for (id, val) in [
        (8000, "6301"),
        (3000, "4711"),
        (3101, "Müller"),
        (3102, "Jürgen"),
        (3103, "07031965"),
        (3110, "1"),
    ] {
        let enc: Vec<u8> = val.chars().map(|c| c as u8).collect();
        v.extend(format!("{:03}{id:04}", 9 + enc.len()).bytes());
        v.extend(enc);
        v.extend(b"\r\n");
    }
    v
}

fn wait_for_worklist(r: &opengdt_bridge::Running) {
    let start = Instant::now();
    while r.worklist.len().unwrap() == 0 {
        assert!(
            start.elapsed() < Duration::from_secs(15),
            "Worklist-Eintrag fehlt"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Leerer Schlüssel („Return Key“, Universal Matching).
fn key(ds: &mut Dataset, g: u16, e: u16, vr: Vr) {
    ds.set_bytes(Tag(g, e), vr, vec![]);
}

/// Die Worklist-Abfrage des DP-70/DP-50 laut Tabelle 9 des Conformance Statements.
fn dp_worklist_query(station_ae: &str, date: &str) -> Dataset {
    let mut q = Dataset::new();
    // Patient: Name (S,*) und ID (S) als Matching-Keys, leer = alle
    for (g, e, vr) in [
        (0x0008, 0x0005, Vr::CS),
        (0x0010, 0x0010, Vr::PN),
        (0x0010, 0x0020, Vr::LO),
        (0x0010, 0x1000, Vr::LO),
        (0x0010, 0x0030, Vr::DA),
        (0x0010, 0x0032, Vr::TM),
        (0x0010, 0x0040, Vr::CS),
        (0x0010, 0x1010, Vr::AS),
        (0x0010, 0x1020, Vr::DS),
        (0x0010, 0x1030, Vr::DS),
        (0x0010, 0x2160, Vr::SH),
        (0x0010, 0x4000, Vr(*b"LT")),
        (0x0040, 0x3001, Vr::LO),
        (0x0010, 0x2000, Vr::LO),
        (0x0010, 0x2110, Vr::LO),
        (0x0010, 0x21B0, Vr(*b"LT")),
        (0x0010, 0x21C0, Vr::US),
        (0x0010, 0x21D0, Vr::DA),
        (0x0038, 0x0050, Vr::LO),
        (0x0038, 0x0500, Vr::LO),
        (0x0038, 0x0010, Vr::LO),
        (0x0038, 0x0300, Vr::LO),
        (0x0008, 0x1080, Vr::LO),
        (0x0020, 0x000D, Vr::UI),
        (0x0032, 0x1060, Vr::LO),
        (0x0040, 0x1001, Vr::SH),
        (0x0040, 0x1003, Vr::SH),
        (0x0040, 0x1004, Vr::LO),
        (0x0040, 0x1010, Vr::PN),
        (0x0040, 0x1400, Vr(*b"LT")),
        (0x0008, 0x0050, Vr::SH),
        (0x0008, 0x0090, Vr::PN),
        (0x0032, 0x1032, Vr::PN),
        (0x0032, 0x1033, Vr::LO),
        (0x0040, 0x2400, Vr(*b"LT")),
    ] {
        key(&mut q, g, e, vr);
    }

    // Sequenzen als Return Keys: ein Item mit leeren Unter-Schlüsseln.
    let code_item = |q: &mut Dataset, g: u16, e: u16| {
        let mut item = Dataset::new();
        for (cg, ce, vr) in [
            (0x0008, 0x0100, Vr::SH),
            (0x0008, 0x0102, Vr::SH),
            (0x0008, 0x0103, Vr::SH),
            (0x0008, 0x0104, Vr::LO),
        ] {
            key(&mut item, cg, ce, vr);
        }
        q.set_seq(Tag(g, e), vec![item]);
    };
    code_item(&mut q, 0x0032, 0x1064); // Requested Procedure Code Sequence
    let mut refs = Dataset::new();
    key(&mut refs, 0x0008, 0x1150, Vr::UI);
    key(&mut refs, 0x0008, 0x1155, Vr::UI);
    q.set_seq(Tag(0x0008, 0x1110), vec![refs]); // Referenced Study Sequence
    q.set_seq(Tag(0x0008, 0x1120), vec![Dataset::new()]); // Referenced Patient Sequence

    // Scheduled Procedure Step Sequence: Modality=US, Station-AE und Datum als Matching-Keys
    let mut sps = Dataset::new();
    sps.set_str(Tag(0x0008, 0x0060), "US");
    sps.set_str_vr(Tag(0x0040, 0x0001), Vr::AE, station_ae.as_bytes().to_vec());
    sps.set_str_vr(Tag(0x0040, 0x0002), Vr::DA, date.as_bytes().to_vec());
    for (g, e, vr) in [
        (0x0032, 0x1070, Vr::LO),
        (0x0040, 0x0003, Vr::TM),
        (0x0040, 0x0004, Vr::DA),
        (0x0040, 0x0005, Vr::TM),
        (0x0040, 0x0006, Vr::PN),
        (0x0040, 0x0007, Vr::LO),
        (0x0040, 0x0009, Vr::SH),
        (0x0040, 0x0010, Vr::SH),
        (0x0040, 0x0011, Vr::SH),
        (0x0040, 0x0012, Vr::LO),
        (0x0040, 0x0020, Vr::CS),
        (0x0040, 0x0400, Vr(*b"LT")),
    ] {
        key(&mut sps, g, e, vr);
    }
    code_item(&mut sps, 0x0040, 0x0008); // Scheduled Protocol Code Sequence
    q.set_seq(Tag(0x0040, 0x0100), vec![sps]);
    q
}

#[test]
fn dp_worklist_query_finds_the_entry() {
    let root = tmp();
    let r = opengdt_bridge::start(conf(&root, "")).unwrap();
    fs::write(root.join("in/PATIN.GDT"), gdt_request()).unwrap();
    wait_for_worklist(&r);

    // Das Gerät schlägt Implicit LE, Explicit LE und Explicit BE vor (Tabelle 8).
    let ts: &[&str] = &[
        uid::IMPLICIT_VR_LE,
        uid::EXPLICIT_VR_LE,
        uid::EXPLICIT_VR_BE,
    ];
    let mut a = connect(r.dicom_addr, &scu(), &[(uid::MWL_FIND, ts)]).unwrap();
    // Standardverhalten des Geräts: Station-AE = eigener AE-Titel, Datum = heute.
    let (status, matches) = a.find(&dp_worklist_query("DP50", "20261015")).unwrap();
    assert_eq!(status, 0);
    assert_eq!(
        matches.len(),
        1,
        "DP-70/DP-50 würde sonst eine leere Worklist anzeigen"
    );
    let m = &matches[0];
    assert_eq!(m.get_str(Tag(0x0010, 0x0020)).as_deref(), Some("4711"));
    assert_eq!(
        m.get_str(Tag(0x0010, 0x0010)).as_deref(),
        Some("Müller^Jürgen")
    );
    assert!(
        m.get_str(Tag(0x0020, 0x000D))
            .is_some_and(|u| u.starts_with("2.25.")),
        "Study Instance UID ist Pflicht"
    );
    a.release().unwrap();
    r.shutdown();
    let _ = fs::remove_dir_all(root);
}

#[test]
fn dp_worklist_query_with_patient_filters() {
    let root = tmp();
    let r = opengdt_bridge::start(conf(&root, "")).unwrap();
    fs::write(root.join("in/PATIN.GDT"), gdt_request()).unwrap();
    wait_for_worklist(&r);
    let ts: &[&str] = &[uid::IMPLICIT_VR_LE];
    let mut a = connect(r.dicom_addr, &scu(), &[(uid::MWL_FIND, ts)]).unwrap();
    for (name, id, expect) in [
        ("M*", "", 1),
        ("Meier*", "", 0),
        ("", "4711", 1),
        ("", "9999", 0),
    ] {
        let mut q = dp_worklist_query("DP50", "20261015");
        q.set_str_vr(Tag(0x0010, 0x0010), Vr::PN, name.as_bytes().to_vec());
        q.set_str_vr(Tag(0x0010, 0x0020), Vr::LO, id.as_bytes().to_vec());
        let (_, m) = a.find(&q).unwrap();
        assert_eq!(m.len(), expect, "Name={name:?} ID={id:?}");
    }
    a.release().unwrap();
    r.shutdown();
    let _ = fs::remove_dir_all(root);
}

const STORAGE_TS_DP: &[&str] = &[
    uid::IMPLICIT_VR_LE,
    uid::EXPLICIT_VR_LE,
    uid::JPEG_BASELINE,
    uid::EXPLICIT_VR_BE,
    "1.2.840.10008.1.2.4.70",
    "1.2.840.10008.1.2.5",
    "1.2.840.10008.1.2.4.90",
    "1.2.840.10008.1.2.4.91",
];

fn negotiated_ts(extra: &str, sop_class: &str) -> Option<String> {
    let root = tmp();
    let r = opengdt_bridge::start(conf(&root, extra)).unwrap();
    let a = connect(r.dicom_addr, &scu(), &[(sop_class, STORAGE_TS_DP)]).unwrap();
    let ts = a.context_for(sop_class).map(|(_, ts)| ts.to_string());
    a.release().unwrap();
    r.shutdown();
    let _ = fs::remove_dir_all(root);
    ts
}

#[test]
fn dp_storage_contexts_are_accepted_for_all_three_sop_classes() {
    for sop in [
        "1.2.840.10008.5.1.4.1.1.6.1",
        "1.2.840.10008.5.1.4.1.1.3.1",
        "1.2.840.10008.5.1.4.1.1.7",
    ] {
        assert!(negotiated_ts("", sop).is_some(), "{sop}");
    }
}

#[test]
fn dp_storage_default_takes_first_proposed_never_big_endian() {
    // Reihenfolge des Geräts: Implicit LE zuerst.
    assert_eq!(
        negotiated_ts("", "1.2.840.10008.5.1.4.1.1.6.1").as_deref(),
        Some(uid::IMPLICIT_VR_LE)
    );
}

#[test]
fn dp_storage_preference_selects_jpeg_when_configured() {
    let ts = negotiated_ts(
        "storage_transfer_syntaxes = jpeg-baseline, explicit-le",
        "1.2.840.10008.5.1.4.1.1.6.1",
    );
    assert_eq!(ts.as_deref(), Some(uid::JPEG_BASELINE));
    // Wird nichts Bevorzugtes angeboten, greift die Reihenfolge der Gegenseite.
    let root = tmp();
    let r =
        opengdt_bridge::start(conf(&root, "storage_transfer_syntaxes = jpeg-baseline")).unwrap();
    let a = connect(
        r.dicom_addr,
        &scu(),
        &[(
            "1.2.840.10008.5.1.4.1.1.6.1",
            &[uid::IMPLICIT_VR_LE, uid::EXPLICIT_VR_LE],
        )],
    )
    .unwrap();
    assert_eq!(
        a.context_for("1.2.840.10008.5.1.4.1.1.6.1").map(|c| c.1),
        Some(uid::IMPLICIT_VR_LE)
    );
    a.release().unwrap();
    r.shutdown();
    let _ = fs::remove_dir_all(root);
}

#[test]
fn dp_storage_commitment_is_not_supported_and_must_be_disabled_on_the_device() {
    let root = tmp();
    let r = opengdt_bridge::start(conf(&root, "")).unwrap();
    let a = connect(
        r.dicom_addr,
        &scu(),
        &[
            ("1.2.840.10008.1.20.1", &[uid::IMPLICIT_VR_LE]),
            (uid::VERIFICATION, &[uid::IMPLICIT_VR_LE]),
        ],
    )
    .unwrap();
    assert!(
        a.context_for("1.2.840.10008.1.20.1").is_none(),
        "Storage Commitment wird nicht angeboten"
    );
    assert!(a.context_for(uid::VERIFICATION).is_some());
    a.release().unwrap();
    r.shutdown();
    let _ = fs::remove_dir_all(root);
}
