//! Watcher: Stabilitätsprüfung, Fehlerfälle und Aufräumverhalten.

use opengdt_bridge::config::Config;
use opengdt_bridge::watcher::Watcher;
use opengdt_dicom::uid;
use opengdt_gdt::Charset;
use opengdt_worklist::WorklistDir;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

struct Env {
    root: PathBuf,
    cfg: Arc<Config>,
    wl: Arc<WorklistDir>,
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn env(extra: &str) -> Env {
    let root = std::env::temp_dir().join(format!("opengdt-watch-{}", uid::generate()));
    fs::create_dir_all(&root).unwrap();
    let cfg = Arc::new(
        Config::parse(&format!(
            "gdt_import_dir = {r}/in\ngdt_export_dir = {r}/out\nworklist_dir = {r}/wl\n{extra}\n",
            r = root.display()
        ))
        .unwrap(),
    );
    let wl = Arc::new(WorklistDir::open(&cfg.worklist_dir).unwrap());
    Env { root, cfg, wl }
}

fn gdt(fields: &[(u16, &str)]) -> Vec<u8> {
    let mut v = Vec::new();
    for (id, val) in fields {
        let enc = Charset::Latin1.encode(val).unwrap();
        v.extend(format!("{:03}{id:04}", 9 + enc.len()).bytes());
        v.extend(enc);
        v.extend(b"\r\n");
    }
    v
}

fn valid() -> Vec<u8> {
    gdt(&[(8000, "6301"), (3000, "1"), (3101, "Test"), (3102, "Anna")])
}

/// Zwei Durchläufe: erster merkt sich, zweiter verarbeitet.
fn scan_twice(w: &mut Watcher) -> usize {
    w.scan() + w.scan()
}

#[test]
fn needs_two_stable_scans() {
    let e = env("");
    let mut w = Watcher::new(e.cfg.clone(), e.wl.clone()).unwrap();
    fs::write(e.cfg.gdt_import_dir.join("a.gdt"), valid()).unwrap();
    assert_eq!(w.scan(), 0, "erster Durchlauf darf noch nichts verarbeiten");
    assert_eq!(e.wl.len().unwrap(), 0);
    assert_eq!(w.scan(), 1);
    assert_eq!(e.wl.len().unwrap(), 1);
}

#[test]
fn file_still_growing_is_not_processed() {
    let e = env("");
    let mut w = Watcher::new(e.cfg.clone(), e.wl.clone()).unwrap();
    let p = e.cfg.gdt_import_dir.join("a.gdt");
    fs::write(&p, &valid()[..10]).unwrap(); // halb geschrieben
    assert_eq!(w.scan(), 0);
    fs::write(&p, valid()).unwrap(); // Größe hat sich geändert
    assert_eq!(
        w.scan(),
        0,
        "geänderte Datei beginnt die Stabilitätsprüfung neu"
    );
    assert_eq!(w.scan(), 1);
}

#[test]
fn uppercase_extension_and_foreign_files() {
    let e = env("");
    let mut w = Watcher::new(e.cfg.clone(), e.wl.clone()).unwrap();
    fs::write(e.cfg.gdt_import_dir.join("PATIN.GDT"), valid()).unwrap();
    fs::write(e.cfg.gdt_import_dir.join("notiz.txt"), b"egal").unwrap();
    fs::create_dir(e.cfg.gdt_import_dir.join("unterordner.gdt")).unwrap();
    assert_eq!(scan_twice(&mut w), 1);
    assert!(
        e.cfg.gdt_import_dir.join("notiz.txt").exists(),
        "fremde Dateien bleiben unberührt"
    );
    assert!(e.cfg.gdt_import_dir.join("unterordner.gdt").is_dir());
}

#[test]
fn keep_processed_moves_file() {
    let e = env("gdt_keep_processed = true");
    let mut w = Watcher::new(e.cfg.clone(), e.wl.clone()).unwrap();
    fs::write(e.cfg.gdt_import_dir.join("a.gdt"), valid()).unwrap();
    assert_eq!(scan_twice(&mut w), 1);
    assert!(!e.cfg.gdt_import_dir.join("a.gdt").exists());
    assert!(e.cfg.gdt_import_dir.join("processed/a.gdt").exists());
}

#[test]
fn wrong_record_type_is_quarantined() {
    let e = env("gdt_request_type = 6301");
    let mut w = Watcher::new(e.cfg.clone(), e.wl.clone()).unwrap();
    fs::write(
        e.cfg.gdt_import_dir.join("falsch.gdt"),
        gdt(&[(8000, "6300"), (3000, "1"), (3101, "X")]),
    )
    .unwrap();
    fs::write(
        e.cfg.gdt_import_dir.join("ohne.gdt"),
        gdt(&[(3000, "2"), (3101, "Y")]),
    )
    .unwrap();
    fs::write(e.cfg.gdt_import_dir.join("richtig.gdt"), valid()).unwrap();
    assert_eq!(scan_twice(&mut w), 1);
    assert!(e.cfg.gdt_import_dir.join("falsch.gdt.err").exists());
    assert!(e.cfg.gdt_import_dir.join("ohne.gdt.err").exists());
    assert_eq!(e.wl.len().unwrap(), 1);
}

#[test]
fn missing_patient_id_and_strict_violations_are_quarantined() {
    let e = env("gdt_strict = true");
    let mut w = Watcher::new(e.cfg.clone(), e.wl.clone()).unwrap();
    fs::write(
        e.cfg.gdt_import_dir.join("keine_id.gdt"),
        gdt(&[(8000, "6301"), (3101, "X")]),
    )
    .unwrap();
    let mut bad_len = valid();
    bad_len[2] = b'9'; // Zeilenlänge falsch
    fs::write(e.cfg.gdt_import_dir.join("laenge.gdt"), bad_len).unwrap();
    assert_eq!(scan_twice(&mut w), 0);
    assert!(e.cfg.gdt_import_dir.join("keine_id.gdt.err").exists());
    assert!(e.cfg.gdt_import_dir.join("laenge.gdt.err").exists());
    // .err-Dateien werden nicht erneut angefasst
    assert_eq!(scan_twice(&mut w), 0);
}

#[test]
fn vanished_import_dir_is_survivable() {
    let e = env("");
    let mut w = Watcher::new(e.cfg.clone(), e.wl.clone()).unwrap();
    fs::remove_dir_all(&e.cfg.gdt_import_dir).unwrap();
    assert_eq!(w.scan(), 0);
    // Verzeichnis taucht wieder auf (z. B. Netzlaufwerk wieder da)
    fs::create_dir_all(&e.cfg.gdt_import_dir).unwrap();
    fs::write(e.cfg.gdt_import_dir.join("a.gdt"), valid()).unwrap();
    assert_eq!(scan_twice(&mut w), 1);
}

#[test]
fn file_deleted_between_scans_is_forgotten() {
    let e = env("");
    let mut w = Watcher::new(e.cfg.clone(), e.wl.clone()).unwrap();
    let p = e.cfg.gdt_import_dir.join("a.gdt");
    fs::write(&p, valid()).unwrap();
    w.scan();
    fs::remove_file(&p).unwrap();
    assert_eq!(w.scan(), 0);
    fs::write(&p, valid()).unwrap();
    assert_eq!(
        w.scan(),
        0,
        "neu angelegte Datei muss die Stabilitätsprüfung neu durchlaufen"
    );
}

#[test]
fn same_patient_twice_replaces_worklist_entry() {
    let e = env("");
    let mut w = Watcher::new(e.cfg.clone(), e.wl.clone()).unwrap();
    for name in ["a.gdt", "b.gdt"] {
        fs::write(e.cfg.gdt_import_dir.join(name), valid()).unwrap();
    }
    assert_eq!(scan_twice(&mut w), 2);
    assert_eq!(e.wl.len().unwrap(), 1);
}

#[test]
fn request_that_cannot_be_removed_is_not_processed_over_and_over() {
    // Verschieben nach processed/ scheitert, weil dort eine Datei im Weg liegt (stellvertretend für fehlende Rechte).
    let e = env("gdt_keep_processed = true");
    let mut w = Watcher::new(e.cfg.clone(), e.wl.clone()).unwrap();
    fs::write(e.cfg.gdt_import_dir.join("processed"), b"im Weg").unwrap();
    fs::write(e.cfg.gdt_import_dir.join("a.gdt"), valid()).unwrap();
    let mut created = 0;
    for _ in 0..12 {
        created += w.scan();
    }
    assert_eq!(
        created, 1,
        "dieselbe Datei darf nicht immer wieder neue Worklist-Einträge erzeugen"
    );
    assert!(
        e.cfg.gdt_import_dir.join("a.gdt").exists(),
        "Datei bleibt liegen (Fehler wird geloggt)"
    );

    // Eine *neue* Datei mit demselben Namen wird dagegen wieder verarbeitet.
    let p = e.cfg.gdt_import_dir.join("a.gdt");
    fs::write(&p, gdt(&[(8000, "6301"), (3000, "2"), (3101, "Zweite")])).unwrap();
    let mut again = 0;
    for _ in 0..4 {
        again += w.scan();
    }
    assert_eq!(again, 1);
    assert_eq!(e.wl.len().unwrap(), 2);
}
