//! BridgeHandler direkt (ohne Netzwerk): Worklist-Abfragen und Store-Statuscodes.

use opengdt_bridge::config::Config;
use opengdt_bridge::handler::BridgeHandler;
use opengdt_bridge::pipeline::Pipeline;
use opengdt_dicom::scp::{Handler, PeerInfo, StoredObject};
use opengdt_dicom::{t, uid, Dataset};
use opengdt_gdt::{Patient, Sex};
use opengdt_store::Archive;
use opengdt_worklist::{build_entry, EntryOptions, WorklistDir};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

struct Env {
    root: PathBuf,
    cfg: Arc<Config>,
    handler: BridgeHandler,
    pipeline: Arc<Pipeline>,
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn env(extra: &str) -> Env {
    let root = std::env::temp_dir().join(format!("opengdt-handler-{}", uid::generate()));
    fs::create_dir_all(&root).unwrap();
    let cfg = Arc::new(
        Config::parse(&format!(
            "worklist_dir = {r}/wl\narchive_dir = {r}/arch\nexport_dir = {r}/export\n\
             gdt_import_dir = {r}/in\ngdt_export_dir = {r}/out\n{extra}\n",
            r = root.display()
        ))
        .unwrap(),
    );
    let worklist = Arc::new(WorklistDir::open(&cfg.worklist_dir).unwrap());
    let archive = Arc::new(Archive::open(&cfg.archive_dir).unwrap());
    let pipeline = Arc::new(Pipeline::new(cfg.clone()));
    let handler = BridgeHandler {
        cfg: cfg.clone(),
        worklist,
        archive,
        pipeline: pipeline.clone(),
    };
    Env {
        root,
        cfg,
        handler,
        pipeline,
    }
}

fn peer() -> PeerInfo {
    PeerInfo {
        calling_ae: "MINDRAY".into(),
        called_ae: "OPENGDT".into(),
        addr: "127.0.0.1:1".into(),
    }
}

fn add_patient(e: &Env, id: &str, name: &str) {
    let p = Patient {
        id: id.into(),
        last_name: name.into(),
        first_name: "X".into(),
        birth_date: None,
        sex: Sex::Unknown,
    };
    let opts = EntryOptions {
        station_ae: e.cfg.station_ae.clone(),
        ..Default::default()
    };
    e.handler.worklist.add(&build_entry(&p, &opts)).unwrap();
}

fn query(station: &str) -> Dataset {
    let mut sps = Dataset::new();
    sps.set_str(t::MODALITY, "US");
    sps.set_str(t::SPS_STATION_AE, station);
    let mut q = Dataset::new();
    q.set_str(t::PATIENT_ID, "");
    q.set_seq(t::SPS_SEQUENCE, vec![sps]);
    q
}

fn instance(patient: &str) -> Dataset {
    let mut ds = Dataset::new();
    ds.set_str(t::PATIENT_ID, patient);
    ds.set_str(t::STUDY_INSTANCE_UID, "1.2.3");
    ds.set_str(t::MODALITY, "US");
    ds
}

fn store_obj(data: Vec<u8>, ts: &str) -> StoredObject {
    StoredObject {
        sop_class: "1.2.840.10008.5.1.4.1.1.6.1".into(),
        sop_instance: "1.2.3.4".into(),
        transfer_syntax: ts.into(),
        data,
    }
}

#[test]
fn handler_advertises_find_and_store() {
    let e = env("");
    assert!(e.handler.supports_find() && e.handler.supports_store());
    e.handler.log("nur Aufruf – darf nicht panicen");
}

#[test]
fn find_without_station_ae_config_matches_any_station() {
    let e = env("");
    add_patient(&e, "1", "Alpha");
    assert_eq!(
        e.handler
            .on_find(&peer(), &query("IRGENDEIN_GERAET"))
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn find_with_configured_station_filters_by_ae() {
    let e = env("station_ae = MINDRAY");
    add_patient(&e, "1", "Alpha");
    assert_eq!(
        e.handler.on_find(&peer(), &query("MINDRAY")).unwrap().len(),
        1
    );
    assert!(e
        .handler
        .on_find(&peer(), &query("ANDERES"))
        .unwrap()
        .is_empty());
}

#[test]
fn find_without_sps_sequence_works() {
    let e = env("");
    add_patient(&e, "1", "Alpha");
    let mut q = Dataset::new();
    q.set_str(t::PATIENT_ID, "");
    assert_eq!(e.handler.on_find(&peer(), &q).unwrap().len(), 1);
}

#[test]
fn find_reports_processing_failure_when_worklist_unreadable() {
    let e = env("");
    fs::remove_dir_all(&e.cfg.worklist_dir).unwrap();
    assert_eq!(e.handler.on_find(&peer(), &query("X")), Err(0x0110));
}

#[test]
fn store_success_archives_and_queues() {
    let e = env("");
    let status = e.handler.on_store(
        &peer(),
        store_obj(instance("4711").encode(true), uid::EXPLICIT_VR_LE),
    );
    assert_eq!(status, 0);
    assert_eq!(e.pipeline.pending_studies(), 1);
}

#[test]
fn store_unparsable_data_is_cannot_understand() {
    let e = env("");
    // abgeschnittenes Element → Dicom-Fehler
    let status = e.handler.on_store(
        &peer(),
        store_obj(
            vec![0x10, 0, 0x10, 0, b'P', b'N', 0xFF, 0xFF],
            uid::EXPLICIT_VR_LE,
        ),
    );
    assert_eq!(status, 0xC000);
    // nicht unterstützte Transfer Syntax
    let status = e
        .handler
        .on_store(&peer(), store_obj(vec![], uid::EXPLICIT_VR_BE));
    assert_eq!(status, 0xC000);
    assert_eq!(e.pipeline.pending_studies(), 0);
}

#[test]
fn store_io_failure_is_out_of_resources() {
    let e = env("");
    // Archiv-Wurzel durch eine Datei ersetzen → Verzeichnisse lassen sich nicht anlegen
    fs::remove_dir_all(&e.cfg.archive_dir).unwrap();
    fs::write(&e.cfg.archive_dir, b"keine Verzeichnis").unwrap();
    let status = e.handler.on_store(
        &peer(),
        store_obj(instance("4711").encode(true), uid::EXPLICIT_VR_LE),
    );
    assert_eq!(status, 0xA700);
    assert_eq!(e.pipeline.pending_studies(), 0);
}
