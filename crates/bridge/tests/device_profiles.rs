//! Prüft die Bridge gegen **jedes Geräteprofil** unter `devices/*/dicom-conformance.json`
//! (aktuell Mindray DP-70/DP-50 und Consona N6–N9).
//!
//! Alle Erwartungen werden aus den Profilen abgeleitet (Worklist-Schlüssel, vorgeschlagene
//! Transfer Syntaxes, SOP-Klassen). Wird eine Datei aus einer neueren Fassung des
//! Herstellerdokuments neu erzeugt, laufen diese Tests automatisch gegen den neuen Stand;
//! ein neuer Ordner mit Profil wird ohne Codeänderung mitgeprüft.

use opengdt_bridge::config::Config;
use opengdt_dicom::scu::{connect, ScuConfig};
use opengdt_dicom::{uid, Dataset, Tag, Vr};
use opengdt_profile::{Key, Profile};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const DEVICES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../devices");

/// Alle Profile unter `devices/*/dicom-conformance.json`, nach Ordnername sortiert.
fn profiles() -> Vec<(String, Profile)> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(DEVICES)
        .expect("devices/ fehlt")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|d| d.join("dicom-conformance.json").is_file())
        .collect();
    dirs.sort();
    assert!(!dirs.is_empty(), "kein Geräteprofil gefunden");
    dirs.into_iter()
        .map(|d| {
            let id = d.file_name().unwrap().to_string_lossy().into_owned();
            let p = Profile::load(d.join("dicom-conformance.json"))
                .unwrap_or_else(|e| panic!("Profil {id} ungültig:\n{}", e.join("\n")));
            (id, p)
        })
        .collect()
}

/// Macht aus `fn name(id, p) { … }` einen Test, der für jedes Profil läuft.
macro_rules! per_profile {
    ($(fn $name:ident($id:ident, $p:ident) $body:block)*) => {$(
        #[test]
        fn $name() {
            for ($id, $p) in profiles() {
                eprintln!("Profil {}", $id);
                $body
            }
        }
    )*};
}

fn tmp() -> PathBuf {
    let d = std::env::temp_dir().join(format!("opengdt-dev-{}", uid::generate()));
    fs::create_dir_all(&d).unwrap();
    d
}

fn conf(root: &Path, extra: &str) -> Config {
    Config::parse(&format!(
        "listen = 127.0.0.1:0\ngdt_import_dir = {r}/in\ngdt_export_dir = {r}/out\nworklist_dir = {r}/wl\n\
         archive_dir = {r}/arch\nexport_dir = {r}/export\n{extra}\n",
        r = root.display()
    ))
    .unwrap()
}

const DEVICE_AE: &str = "DEVICE1";

fn scu() -> ScuConfig {
    ScuConfig {
        calling_ae: DEVICE_AE.into(),
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

/// Startet die Bridge und legt den Patienten 4711 in die Worklist.
fn bridge_with_patient(root: &Path, extra: &str) -> opengdt_bridge::Running {
    let r = opengdt_bridge::start(conf(root, extra)).unwrap();
    fs::write(root.join("in/PATIN.GDT"), gdt_request()).unwrap();
    let start = Instant::now();
    while r.worklist.len().unwrap() == 0 {
        assert!(
            start.elapsed() < Duration::from_secs(15),
            "Worklist-Eintrag fehlt"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    r
}

fn vr(k: &Key) -> Vr {
    Vr(k.vr)
}

/// Baut aus den Profil-Schlüsseln einen Query-Datensatz. `overrides` setzt Suchwerte
/// (z. B. Name, Station-AE, Datum); alle anderen Schlüssel bleiben leer (Return Keys).
fn build_query(keys: &[Key], overrides: &HashMap<(u16, u16), String>) -> Dataset {
    let mut q = Dataset::new();
    for k in keys {
        let tag = Tag(k.tag.0, k.tag.1);
        if &k.vr == b"SQ" {
            q.set_seq(tag, vec![build_query(&k.items, overrides)]);
        } else {
            let v = overrides
                .get(&k.tag)
                .cloned()
                .or_else(|| k.fixed_value.clone())
                .unwrap_or_default();
            q.set_str_vr(tag, vr(k), v.into_bytes());
        }
    }
    q
}

fn ids(list: &[opengdt_profile::TransferSyntax]) -> Vec<&str> {
    list.iter().map(|t| t.uid.as_str()).collect()
}

per_profile! {
fn device_worklist_query_finds_the_entry(id, p) {
    let root = tmp();
    let r = bridge_with_patient(&root, "");
    let ts = p.worklist_transfer_syntaxes();
    let mut a = connect(r.dicom_addr, &scu(), &[(uid::MWL_FIND, &ids(&ts))]).unwrap();
    // Standardverhalten laut Profil: Station-AE = eigener AE-Titel, Datum = heute
    let o = HashMap::from([
        ((0x0040, 0x0001), DEVICE_AE.to_string()),
        ((0x0040, 0x0002), "20261015".to_string()),
    ]);
    let (status, matches) = a.find(&build_query(&p.worklist_keys(), &o)).unwrap();
    assert_eq!(status, 0);
    assert_eq!(
        matches.len(),
        1,
        "das Gerät würde sonst eine leere Worklist anzeigen"
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
}

per_profile! {
fn every_single_profile_key_is_accepted_as_return_key(id, p) {
    // Jeder Schlüssel des Profils einzeln (leer = Universal Matching) darf den Treffer nie verhindern.
    let root = tmp();
    let r = bridge_with_patient(&root, "");
    let ts = p.worklist_transfer_syntaxes();
    for implicit_only in [true, false] {
        let proposed: Vec<&str> = if implicit_only {
            vec![uid::IMPLICIT_VR_LE]
        } else {
            ids(&ts)
        };
        let mut a = connect(r.dicom_addr, &scu(), &[(uid::MWL_FIND, &proposed)]).unwrap();
        for k in p.worklist_keys() {
            // Schlüssel mit festem Wert (z. B. Modality=US) gehören in die SPS-Sequenz und werden dort geprüft.
            if k.fixed_value.is_some() {
                continue;
            }
            let q = build_query(std::slice::from_ref(&k), &HashMap::new());
            let (status, m) = a.find(&q).unwrap();
            assert_eq!(
                (status, m.len()),
                (0, 1),
                "Schlüssel {} {:04X},{:04X} (implicit={implicit_only})",
                k.name,
                k.tag.0,
                k.tag.1
            );
        }
        a.release().unwrap();
    }
    r.shutdown();
    let _ = fs::remove_dir_all(root);
}
}

per_profile! {
fn device_worklist_query_with_patient_filters(id, p) {
    let root = tmp();
    let r = bridge_with_patient(&root, "");
    let a_ts = [uid::IMPLICIT_VR_LE];
    let mut a = connect(r.dicom_addr, &scu(), &[(uid::MWL_FIND, &a_ts)]).unwrap();
    for (name, id, expect) in [
        ("M*", "", 1),
        ("Meier*", "", 0),
        ("", "4711", 1),
        ("", "9999", 0),
    ] {
        let o = HashMap::from([
            ((0x0010, 0x0010), name.to_string()),
            ((0x0010, 0x0020), id.to_string()),
            ((0x0040, 0x0001), DEVICE_AE.to_string()),
        ]);
        let (_, m) = a.find(&build_query(&p.worklist_keys(), &o)).unwrap();
        assert_eq!(m.len(), expect, "Name={name:?} ID={id:?}");
    }
    a.release().unwrap();
    r.shutdown();
    let _ = fs::remove_dir_all(root);
}
}

/// Erste Transfer Syntax der Liste, die die Bridge überhaupt annimmt (nie Big Endian/Deflate).
fn first_acceptable(list: &[&str]) -> String {
    list.iter()
        .find(|u| !matches!(**u, uid::EXPLICIT_VR_BE | uid::DEFLATED_EXPLICIT_VR_LE))
        .expect("Profil bietet keine annehmbare Transfer Syntax")
        .to_string()
}

fn negotiate(extra: &str, sop: &str, proposed: &[&str]) -> Option<String> {
    let root = tmp();
    let r = opengdt_bridge::start(conf(&root, extra)).unwrap();
    let a = connect(r.dicom_addr, &scu(), &[(sop, proposed)]).unwrap();
    let ts = a.context_for(sop).map(|(_, ts)| ts.to_string());
    a.release().unwrap();
    r.shutdown();
    let _ = fs::remove_dir_all(root);
    ts
}

per_profile! {
fn every_storage_context_of_the_device_is_accepted_with_default_policy(id, p) {
    let contexts = p.storage_contexts();
    assert!(!contexts.is_empty());
    for c in &contexts {
        let proposed = ids(&c.transfer_syntaxes);
        let got = negotiate("", &c.uid, &proposed);
        assert_eq!(
            got,
            Some(first_acceptable(&proposed)),
            "{} ({})",
            c.name,
            c.uid
        );
    }
}
}

per_profile! {
fn every_storage_sop_class_marked_scu_is_accepted(id, p) {
    let root = tmp();
    let r = opengdt_bridge::start(conf(&root, "")).unwrap();
    for c in p
        .sop_classes()
        .into_iter()
        .filter(|c| c.scu && uid::is_storage_sop_class(&c.uid))
    {
        let a = connect(r.dicom_addr, &scu(), &[(&c.uid, &[uid::IMPLICIT_VR_LE])]).unwrap();
        assert!(a.context_for(&c.uid).is_some(), "{} ({})", c.name, c.uid);
        a.release().unwrap();
    }
    r.shutdown();
    let _ = fs::remove_dir_all(root);
}
}

per_profile! {
fn storage_preference_from_config_wins_over_device_order(id, p) {
    for c in p.storage_contexts() {
        let proposed = ids(&c.transfer_syntaxes);
        if proposed.contains(&uid::JPEG_BASELINE) {
            let got = negotiate(
                "storage_transfer_syntaxes = jpeg-baseline, explicit-le",
                &c.uid,
                &proposed,
            );
            assert_eq!(got.as_deref(), Some(uid::JPEG_BASELINE), "{}", c.name);
        }
    }
}
}

per_profile! {
fn storage_commitment_is_not_supported_and_must_be_disabled_on_the_device(id, p) {
    let sc = p
        .find_sop_class("commitment")
        .expect("Profil enthält Storage Commitment");
    assert!(
        sc.scu,
        "laut Profil spricht das Gerät Storage Commitment als SCU"
    );
    let root = tmp();
    let r = opengdt_bridge::start(conf(&root, "")).unwrap();
    let a = connect(
        r.dicom_addr,
        &scu(),
        &[
            (&sc.uid, &[uid::IMPLICIT_VR_LE]),
            (uid::VERIFICATION, &[uid::IMPLICIT_VR_LE]),
        ],
    )
    .unwrap();
    assert!(
        a.context_for(&sc.uid).is_none(),
        "Bridge bietet kein Storage Commitment an"
    );
    assert!(a.context_for(uid::VERIFICATION).is_some());
    a.release().unwrap();
    r.shutdown();
    let _ = fs::remove_dir_all(root);
}
}

per_profile! {
fn default_query_with_empty_station_ae_finds_the_entry(id, p) {
    // Die Consona-Reihe fragt standardmäßig mit LEEREM Scheduled Station AE Title (das DP-50 mit dem eigenen).
    // Leer ist Universal Matching: muss sowohl ohne als auch mit konfiguriertem station_ae treffen.
    for extra in ["", "station_ae = DEVICE1"] {
        let root = tmp();
        let r = bridge_with_patient(&root, extra);
        let ts = p.worklist_transfer_syntaxes();
        let mut a = connect(r.dicom_addr, &scu(), &[(uid::MWL_FIND, &ids(&ts))]).unwrap();
        let o = HashMap::from([((0x0040, 0x0001), String::new())]);
        let (status, matches) = a.find(&build_query(&p.worklist_keys(), &o)).unwrap();
        assert_eq!(status, 0, "{id}");
        assert_eq!(matches.len(), 1, "{id}: leerer Station-AE ({extra:?}) muss treffen");
        a.release().unwrap();
        r.shutdown();
        let _ = fs::remove_dir_all(root);
    }
}
}

per_profile! {
fn mpps_is_not_supported_and_must_not_be_configured_on_the_device(id, p) {
    // Gilt nur für Profile, in denen das Gerät MPPS als SCU spricht (neuere Geräte).
    let Some(mpps) = p.find_sop_class("performed procedure") else {
        eprintln!("{id}: kein MPPS im Profil");
        continue;
    };
    assert!(mpps.scu, "{id}: laut Profil spricht das Gerät MPPS als SCU");
    let root = tmp();
    let r = opengdt_bridge::start(conf(&root, "")).unwrap();
    let a = connect(
        r.dicom_addr,
        &scu(),
        &[
            (&mpps.uid, &[uid::IMPLICIT_VR_LE]),
            (uid::VERIFICATION, &[uid::IMPLICIT_VR_LE]),
        ],
    )
    .unwrap();
    assert!(
        a.context_for(&mpps.uid).is_none(),
        "{id}: Bridge bietet kein MPPS an"
    );
    assert!(a.context_for(uid::VERIFICATION).is_some());
    a.release().unwrap();
    r.shutdown();
    let _ = fs::remove_dir_all(root);
}
}

/// Dieselbe Geräteabfrage gegen einen **externen** Worklist-Server (z. B. Orthanc), falls
/// `OPENGDT_EXTERNAL_MWL=host:port` gesetzt ist (`tests/orthanc/run.sh` tut das). Sonst übersprungen.
/// Optional: `OPENGDT_EXTERNAL_MWL_AE` (Standard ORTHANC), `OPENGDT_STATION_AE` (Standard MINDRAY),
/// `OPENGDT_QUERY_DATE` (YYYYMMDD; ohne Angabe wird nicht nach Datum gefragt).
#[test]
fn device_query_against_external_worklist_server() {
    let Ok(addr) = std::env::var("OPENGDT_EXTERNAL_MWL") else {
        eprintln!("übersprungen: OPENGDT_EXTERNAL_MWL nicht gesetzt");
        return;
    };
    for (id, p) in profiles() {
        eprintln!("Profil {id}");
        external_query(&addr, &p);
    }
}

fn external_query(addr: &str, p: &Profile) {
    let called = std::env::var("OPENGDT_EXTERNAL_MWL_AE").unwrap_or_else(|_| "ORTHANC".into());
    let station = std::env::var("OPENGDT_STATION_AE").unwrap_or_else(|_| "MINDRAY".into());
    let date = std::env::var("OPENGDT_QUERY_DATE").unwrap_or_default();
    let cfg = ScuConfig {
        calling_ae: station.clone(),
        called_ae: called,
        timeout: Duration::from_secs(10),
    };
    let ts = p.worklist_transfer_syntaxes();
    let mut a = connect(addr, &cfg, &[(uid::MWL_FIND, &ids(&ts))]).unwrap();

    // 1. Volle Standardabfrage des Geräts
    let o = HashMap::from([
        ((0x0040, 0x0001), station.clone()),
        ((0x0040, 0x0002), date.clone()),
    ]);
    let (status, m) = a.find(&build_query(&p.worklist_keys(), &o)).unwrap();
    assert_eq!(status, 0, "Statuscode des Servers");
    assert!(!m.is_empty(), "externer Server liefert keinen Treffer – Worklist-Eintrag vorhanden? Station-AE/Datum passend?");
    let w = &m[0];
    assert_eq!(w.get_str(Tag(0x0010, 0x0020)).as_deref(), Some("4711"));
    assert_eq!(
        w.get_str(Tag(0x0010, 0x0010)).as_deref(),
        Some("Müller^Jürgen")
    );
    assert!(
        w.get_str(Tag(0x0020, 0x000D)).is_some(),
        "Study Instance UID fehlt"
    );
    assert!(
        w.get_str(Tag(0x0008, 0x0050)).is_some(),
        "Accession Number fehlt"
    );
    a.release().unwrap();

    // 2. Jeder Schlüssel einzeln, in beiden Kodierungen
    for implicit_only in [true, false] {
        let proposed: Vec<&str> = if implicit_only {
            vec![uid::IMPLICIT_VR_LE]
        } else {
            ids(&ts)
        };
        let mut a = connect(addr, &cfg, &[(uid::MWL_FIND, &proposed)]).unwrap();
        for k in p.worklist_keys() {
            if k.fixed_value.is_some() {
                continue;
            }
            let q = build_query(std::slice::from_ref(&k), &HashMap::new());
            let (status, m) = a.find(&q).unwrap();
            assert!(status == 0 && !m.is_empty(), "Schlüssel {} {:04X},{:04X} (implicit={implicit_only}): status={status:#06x}, Treffer={}", k.name, k.tag.0, k.tag.1, m.len());
        }
        a.release().unwrap();
    }
}
