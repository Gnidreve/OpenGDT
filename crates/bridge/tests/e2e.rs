//! Gesamter Ablauf: GDT-Datei → Worklist → C-FIND → C-STORE → Dateien + GDT-Antwort.

use opengdt_bridge::config::Config;
use opengdt_dicom::dataset::{Element, Value};
use opengdt_dicom::scu::{connect, ScuConfig};
use opengdt_dicom::{t, uid, Dataset, Vr};
use opengdt_gdt::{Charset, Record};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn wait_for<T>(what: &str, mut f: impl FnMut() -> Option<T>) -> T {
    let start = Instant::now();
    loop {
        if let Some(v) = f() {
            return v;
        }
        assert!(start.elapsed() < Duration::from_secs(15), "Timeout: {what}");
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn gdt_request() -> Vec<u8> {
    let mut v = Vec::new();
    for (id, val) in [
        (8000, "6301"),
        (9206, "3"),
        (3000, "4711"),
        (3101, "Müller"),
        (3102, "Jürgen"),
        (3103, "07031965"),
        (3110, "1"),
    ] {
        let enc = Charset::Latin1.encode(val).unwrap();
        v.extend(format!("{:03}{id:04}", 9 + enc.len()).bytes());
        v.extend(enc);
        v.extend(b"\r\n");
    }
    v
}

fn ultrasound_instance(patient_id: &str, study: &str, sop: &str, nr: u32) -> Dataset {
    let mut ds = Dataset::new();
    ds.set_str(t::SOP_CLASS_UID, "1.2.840.10008.5.1.4.1.1.6.1");
    ds.set_str(t::SOP_INSTANCE_UID, sop);
    ds.set_str(t::PATIENT_NAME, "Müller^Jürgen");
    ds.set_str(t::PATIENT_ID, patient_id);
    ds.set_str(t::PATIENT_BIRTH_DATE, "19650307");
    ds.set_str(t::PATIENT_SEX, "M");
    ds.set_str(t::STUDY_INSTANCE_UID, study);
    ds.set_str(t::STUDY_DATE, "20261015");
    ds.set_str(t::STUDY_TIME, "103000");
    ds.set_str(t::ACCESSION_NUMBER, "ACC1");
    ds.set_str(t::MODALITY, "US");
    ds.set_str(t::INSTANCE_NUMBER, &nr.to_string());
    ds.insert(Element {
        tag: t::PIXEL_DATA,
        vr: Vr::OB,
        value: Value::Fragments(vec![vec![], vec![0xFF, 0xD8, nr as u8, 0xFF, 0xD9]]),
    });
    ds
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("opengdt-e2e-{name}-{}", uid::generate()));
    fs::create_dir_all(&d).unwrap();
    d
}

fn files_with_ext(dir: &Path, ext: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == ext) {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

#[test]
fn full_roundtrip() {
    let root = tmp("full");
    let cfg = Config::parse(&format!(
        "listen = 127.0.0.1:0\n\
         gdt_import_dir = {r}/in\ngdt_export_dir = {r}/out\nworklist_dir = {r}/wl\n\
         archive_dir = {r}/arch\nexport_dir = {r}/export\n\
         settle_seconds = 1\npvs_export_prefix = \\\\SRV\\us\n\
         gdt_result_type = 6310\n",
        r = root.display()
    ))
    .unwrap();
    let running = opengdt_bridge::start(cfg).unwrap();
    let addr = running.dicom_addr;

    // 1. PVS legt Anforderung ab
    fs::write(root.join("in/PATIN.GDT"), gdt_request()).unwrap();
    wait_for("Worklist-Eintrag", || {
        (running.worklist.len().unwrap() == 1).then_some(())
    });
    assert!(
        !root.join("in/PATIN.GDT").exists(),
        "Anforderung muss konsumiert werden"
    );

    // 2. Gerät fragt Worklist ab
    let ts: &[&str] = &[uid::IMPLICIT_VR_LE];
    let scu = ScuConfig {
        calling_ae: "MINDRAY".into(),
        called_ae: "OPENGDT".into(),
        timeout: Duration::from_secs(5),
    };
    let mut a = connect(addr, &scu, &[(uid::MWL_FIND, ts)]).unwrap();
    let mut q = Dataset::new();
    q.set_str(t::PATIENT_NAME, "");
    q.set_str(t::PATIENT_ID, "");
    let mut sps = Dataset::new();
    sps.set_str(t::MODALITY, "US");
    sps.set_str(t::SPS_STATION_AE, "MINDRAY"); // Bridge hat keine station_ae → trotzdem Treffer
    q.set_seq(t::SPS_SEQUENCE, vec![sps]);
    let (status, matches) = a.find(&q).unwrap();
    assert_eq!(status, 0);
    assert_eq!(matches.len(), 1);
    assert_eq!(
        matches[0].get_str(t::PATIENT_NAME).as_deref(),
        Some("Müller^Jürgen")
    );
    assert_eq!(matches[0].get_str(t::PATIENT_ID).as_deref(), Some("4711"));
    a.release().unwrap();

    // 3. Gerät sendet zwei Bilder über zwei getrennte Associations
    let sop_class = "1.2.840.10008.5.1.4.1.1.6.1";
    for (nr, sop) in [(1u32, "1.2.3.1"), (2, "1.2.3.2")] {
        let mut a = connect(addr, &scu, &[(sop_class, &[uid::JPEG_BASELINE])]).unwrap();
        let ds = ultrasound_instance("4711", "1.2.3", sop, nr);
        assert_eq!(a.store(sop_class, sop, &ds.encode(true)).unwrap(), 0);
        a.release().unwrap();
    }

    // 4. Eine einzige GDT-Rückmeldung für beide Bilder
    let gdt_file = wait_for("GDT-Rückmeldung", || {
        files_with_ext(&root.join("out"), "gdt").pop()
    });
    std::thread::sleep(Duration::from_millis(1500));
    assert_eq!(
        files_with_ext(&root.join("out"), "gdt").len(),
        1,
        "nur eine Rückmeldung je Untersuchung"
    );

    let rec = Record::parse(&fs::read(&gdt_file).unwrap(), true).unwrap();
    assert_eq!(rec.raw(8000), Some(&b"6310"[..]));
    assert_eq!(rec.raw(3000), Some(&b"4711"[..]));
    assert_eq!(
        rec.get(3101, Charset::Latin1).unwrap().as_deref(),
        Some("Müller")
    );
    assert_eq!(rec.raw(3103), Some(&b"07031965"[..]));
    assert_eq!(rec.raw(6200), Some(&b"15102026"[..]));
    let refs: Vec<_> = rec.fields.iter().filter(|f| f.id == 6305).collect();
    assert_eq!(refs.len(), 2);
    let first = String::from_utf8(refs[0].value.clone()).unwrap();
    assert!(
        first.starts_with(r"\\SRV\us\4711\20261015_ACC1\001_"),
        "{first}"
    );
    assert!(first.ends_with(".jpg"));

    // Bilder liegen exportiert + Originale archiviert
    let jpgs = files_with_ext(&root.join("export"), "jpg");
    assert_eq!(jpgs.len(), 2);
    assert_eq!(fs::read(&jpgs[0]).unwrap(), vec![0xFF, 0xD8, 1, 0xFF, 0xD9]);
    assert_eq!(files_with_ext(&root.join("arch"), "dcm").len(), 2);

    running.shutdown();
    let _ = fs::remove_dir_all(root);
}

#[test]
fn broken_gdt_is_quarantined_not_looped() {
    let root = tmp("bad");
    let cfg = Config::parse(&format!(
        "listen = 127.0.0.1:0\ngdt_import_dir = {r}/in\ngdt_export_dir = {r}/out\nworklist_dir = {r}/wl\n\
         archive_dir = {r}/arch\nexport_dir = {r}/export\n",
        r = root.display()
    ))
    .unwrap();
    let running = opengdt_bridge::start(cfg).unwrap();
    fs::write(root.join("in/kaputt.gdt"), b"das ist kein gdt").unwrap();
    wait_for("Quarantäne", || {
        root.join("in/kaputt.gdt.err").exists().then_some(())
    });
    assert_eq!(running.worklist.len().unwrap(), 0);
    running.shutdown();
    let _ = fs::remove_dir_all(root);
}

#[test]
fn result_file_is_not_overwritten_while_pvs_has_not_consumed() {
    let root = tmp("fixed");
    let cfg = Config::parse(&format!(
        "listen = 127.0.0.1:0\ngdt_import_dir = {r}/in\ngdt_export_dir = {r}/out\nworklist_dir = {r}/wl\n\
         archive_dir = {r}/arch\nexport_dir = {r}/export\nsettle_seconds = 0\ngdt_result_filename = OPENGDT.GDT\n",
        r = root.display()
    ))
    .unwrap();
    let running = opengdt_bridge::start(cfg).unwrap();
    let scu = ScuConfig {
        calling_ae: "X".into(),
        called_ae: "OPENGDT".into(),
        timeout: Duration::from_secs(5),
    };
    let sop_class = "1.2.840.10008.5.1.4.1.1.6.1";
    let send = |study: &str, sop: &str| {
        let mut a = connect(
            running.dicom_addr,
            &scu,
            &[(sop_class, &[uid::JPEG_BASELINE])],
        )
        .unwrap();
        a.store(
            sop_class,
            sop,
            &ultrasound_instance("1", study, sop, 1).encode(true),
        )
        .unwrap();
        a.release().unwrap();
    };
    send("9.1", "9.1.1");
    wait_for("erste Datei", || {
        root.join("out/OPENGDT.GDT").exists().then_some(())
    });
    let first = fs::read(root.join("out/OPENGDT.GDT")).unwrap();
    send("9.2", "9.2.1");
    std::thread::sleep(Duration::from_millis(2500));
    assert_eq!(
        fs::read(root.join("out/OPENGDT.GDT")).unwrap(),
        first,
        "nicht überschreiben"
    );
    assert_eq!(running.pipeline.pending_studies(), 1);
    // PVS holt ab → zweite Untersuchung wird nachgeliefert
    fs::remove_file(root.join("out/OPENGDT.GDT")).unwrap();
    wait_for("zweite Datei", || {
        root.join("out/OPENGDT.GDT").exists().then_some(())
    });
    assert_ne!(fs::read(root.join("out/OPENGDT.GDT")).unwrap(), first);
    running.shutdown();
    let _ = fs::remove_dir_all(root);
}

#[test]
fn unreported_studies_are_recovered_after_restart() {
    use opengdt_bridge::pipeline::Pipeline;
    use opengdt_dicom::scp::StoredObject;
    use opengdt_store::Archive;
    use std::sync::Arc;

    let root = tmp("recover");
    let cfg = Arc::new(
        Config::parse(&format!(
            "gdt_import_dir = {r}/in\ngdt_export_dir = {r}/out\nworklist_dir = {r}/wl\n\
             archive_dir = {r}/arch\nexport_dir = {r}/export\nsettle_seconds = 0\n",
            r = root.display()
        ))
        .unwrap(),
    );
    // „Vor dem Absturz“ archivierte Instanz, nie an die Pipeline gemeldet.
    let archive = Archive::open(&cfg.archive_dir).unwrap();
    let ds = ultrasound_instance("4711", "7.7", "7.7.1", 1);
    archive
        .save(&StoredObject {
            sop_class: "1.2.840.10008.5.1.4.1.1.6.1".into(),
            sop_instance: "7.7.1".into(),
            transfer_syntax: uid::JPEG_BASELINE.into(),
            data: ds.encode(true),
        })
        .unwrap();

    let p = Pipeline::new(cfg.clone());
    assert_eq!(p.recover(&cfg.archive_dir), 1);
    assert_eq!(p.flush(true), 1);
    assert_eq!(files_with_ext(&root.join("out"), "gdt").len(), 1);

    // Zweiter Start: Marker verhindert doppelte Rückmeldung.
    let p2 = Pipeline::new(cfg.clone());
    assert_eq!(p2.recover(&cfg.archive_dir), 0);
    let _ = fs::remove_dir_all(root);
}
