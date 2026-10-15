//! Orthanc-Poller gegen einen Fake-Orthanc (Fehlerfälle). Das normale Verhalten gegen ein ECHTES Orthanc
//! prüft `tests/orthanc/run.sh`.

use opengdt_bridge::config::Config;
use opengdt_bridge::orthanc::Poller;
use opengdt_bridge::pipeline::Pipeline;
use opengdt_dicom::dataset::{Element, Value};
use opengdt_dicom::file::write_part10;
use opengdt_dicom::{t, uid, Dataset, Vr};
use opengdt_store::Archive;
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct FakeState {
    changes: Vec<(u64, String, String)>, // (Seq, Typ, ID)
    last: u64,
    studies: HashMap<String, Vec<String>>,
    files: HashMap<String, Vec<u8>>,
    fail_files: bool,
    /// Nur diese Instanzen liefern dauerhaft HTTP 500.
    fail_instances: Vec<String>,
    auth_required: Option<String>,
    requests: Vec<(String, Option<String>)>,
}

struct Fake {
    addr: SocketAddr,
    state: Arc<Mutex<FakeState>>,
    stop: Arc<AtomicBool>,
}

impl Drop for Fake {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn respond(s: &mut TcpStream, status: &str, body: &[u8]) {
    let _ = write!(
        s,
        "HTTP/1.0 {status}\r\nContent-Length: {}\r\n\r\n",
        body.len()
    );
    let _ = s.write_all(body);
}

fn handle(mut s: TcpStream, st: &Mutex<FakeState>) {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
        match s.read(&mut chunk) {
            Ok(0) | Err(_) => return,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    }
    let text = String::from_utf8_lossy(&buf).to_string();
    let path = text
        .lines()
        .next()
        .unwrap_or("")
        .split_whitespace()
        .nth(1)
        .unwrap_or("")
        .to_string();
    let auth = text
        .lines()
        .find_map(|l| l.strip_prefix("Authorization: "))
        .map(String::from);
    let mut st = st.lock().unwrap();
    st.requests.push((path.clone(), auth.clone()));
    if st.auth_required.is_some() && st.auth_required != auth {
        return respond(&mut s, "401 Unauthorized", b"");
    }
    if path == "/changes?last" {
        let body = format!(r#"{{"Changes":[],"Done":true,"Last":{}}}"#, st.last);
        return respond(&mut s, "200 OK", body.as_bytes());
    }
    if let Some(q) = path.strip_prefix("/changes?since=") {
        let (since, limit) = q.split_once("&limit=").unwrap();
        let (since, limit): (u64, usize) = (since.parse().unwrap(), limit.parse().unwrap());
        let all: Vec<_> = st.changes.iter().filter(|c| c.0 > since).collect();
        let page = &all[..all.len().min(limit)];
        let done = all.len() <= limit;
        let last = if done {
            st.last
        } else {
            page.last().unwrap().0
        };
        let items: Vec<String> = page
            .iter()
            .map(|(seq, kind, id)| {
                format!(
                    r#"{{"Seq":{seq},"ChangeType":"{kind}","ID":"{id}","ResourceType":"Study"}}"#
                )
            })
            .collect();
        let body = format!(
            r#"{{"Changes":[{}],"Done":{done},"Last":{last}}}"#,
            items.join(",")
        );
        return respond(&mut s, "200 OK", body.as_bytes());
    }
    if let Some(id) = path
        .strip_prefix("/studies/")
        .and_then(|p| p.strip_suffix("/instances"))
    {
        return match st.studies.get(id) {
            Some(list) => {
                let items: Vec<String> =
                    list.iter().map(|i| format!(r#"{{"ID":"{i}"}}"#)).collect();
                respond(
                    &mut s,
                    "200 OK",
                    format!("[{}]", items.join(",")).as_bytes(),
                )
            }
            None => respond(&mut s, "404 Not Found", b"{}"),
        };
    }
    if let Some(id) = path
        .strip_prefix("/instances/")
        .and_then(|p| p.strip_suffix("/file"))
    {
        if st.fail_files || st.fail_instances.iter().any(|x| x == id) {
            return respond(&mut s, "500 Internal Server Error", b"");
        }
        return match st.files.get(id) {
            Some(b) => respond(&mut s, "200 OK", b),
            None => respond(&mut s, "404 Not Found", b""),
        };
    }
    respond(&mut s, "404 Not Found", b"");
}

fn fake_at(listener: TcpListener) -> Fake {
    listener.set_nonblocking(true).unwrap();
    let addr = listener.local_addr().unwrap();
    let state = Arc::new(Mutex::new(FakeState::default()));
    let stop = Arc::new(AtomicBool::new(false));
    let (st2, stop2) = (state.clone(), stop.clone());
    std::thread::spawn(move || {
        while !stop2.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((s, _)) => {
                    let _ = s.set_nonblocking(false);
                    handle(s, &st2);
                }
                Err(_) => std::thread::sleep(std::time::Duration::from_millis(5)),
            }
        }
    });
    Fake { addr, state, stop }
}

fn fake() -> Fake {
    fake_at(TcpListener::bind("127.0.0.1:0").unwrap())
}

/// Part-10-Datei eines Ultraschallbilds (JPEG Baseline).
fn instance_file(patient: &str, study: &str, sop: &str) -> Vec<u8> {
    let mut ds = Dataset::new();
    ds.set_str(t::SOP_CLASS_UID, "1.2.840.10008.5.1.4.1.1.6.1");
    ds.set_str(t::SOP_INSTANCE_UID, sop);
    ds.set_str(t::PATIENT_ID, patient);
    ds.set_str(t::PATIENT_NAME, "Test^Anna");
    ds.set_str(t::STUDY_INSTANCE_UID, study);
    ds.set_str(t::STUDY_DATE, "20261015");
    ds.set_str(t::MODALITY, "US");
    ds.set_str(t::INSTANCE_NUMBER, "1");
    ds.insert(Element {
        tag: t::PIXEL_DATA,
        vr: Vr::OB,
        value: Value::Fragments(vec![vec![], vec![0xFF, 0xD8, 1, 0xFF, 0xD9]]),
    });
    write_part10(
        "1.2.840.10008.5.1.4.1.1.6.1",
        sop,
        uid::JPEG_BASELINE,
        &ds.encode(true),
    )
}

impl Fake {
    fn add_study(&self, seq: u64, study: &str, sop: &str) {
        let mut st = self.state.lock().unwrap();
        st.changes.push((seq, "StableStudy".into(), study.into()));
        st.studies.insert(study.into(), vec![format!("inst-{sop}")]);
        st.files.insert(
            format!("inst-{sop}"),
            instance_file("4711", &format!("1.2.{seq}"), sop),
        );
        st.last = st.last.max(seq);
    }
}

struct Env {
    root: PathBuf,
    cfg: Config,
    pipeline: Arc<Pipeline>,
    archive: Arc<Archive>,
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn env(url: &str, extra: &str) -> Env {
    let root = std::env::temp_dir().join(format!("opengdt-orthanc-{}", uid::generate()));
    fs::create_dir_all(&root).unwrap();
    let cfg = Config::parse(&format!(
        "archive_dir = {r}/arch\nexport_dir = {r}/export\ngdt_import_dir = {r}/in\ngdt_export_dir = {r}/out\n\
         worklist_dir = {r}/wl\northanc_url = {url}\n{extra}\n",
        r = root.display()
    ))
    .unwrap();
    let archive = Arc::new(Archive::open(&cfg.archive_dir).unwrap());
    let pipeline = Arc::new(Pipeline::new(Arc::new(cfg.clone())));
    Env {
        root,
        cfg,
        pipeline,
        archive,
    }
}

fn url(f: &Fake) -> String {
    format!("http://{}", f.addr)
}

fn poller(e: &Env) -> Poller {
    Poller::new(&e.cfg, e.archive.clone(), e.pipeline.clone()).unwrap()
}

fn state_file(e: &Env) -> String {
    fs::read_to_string(e.cfg.archive_dir.join(".orthanc-since")).unwrap_or_default()
}

#[test]
fn first_start_skips_history_unless_asked() {
    let f = fake();
    f.add_study(5, "old-1", "a");
    let e = env(&url(&f), "");
    let mut p = poller(&e);
    assert_eq!(
        (p.since, state_file(&e).as_str()),
        (5, "5"),
        "Stand auf die letzte Änderung gesetzt"
    );
    assert_eq!(p.poll_once(), 0, "alte Untersuchung wird nicht übernommen");
    assert_eq!(e.pipeline.pending_studies(), 0);

    let e2 = env(&url(&f), "orthanc_import_history = true");
    let mut p2 = poller(&e2);
    assert_eq!(p2.since, 0);
    assert_eq!(
        p2.poll_once(),
        1,
        "mit import_history wird die Historie übernommen"
    );
}

#[test]
fn ingests_stable_studies_once_and_persists_position() {
    let f = fake();
    let e = env(&url(&f), "");
    fs::create_dir_all(&e.cfg.archive_dir).unwrap();
    fs::write(e.cfg.archive_dir.join(".orthanc-since"), "0").unwrap();
    {
        let mut st = f.state.lock().unwrap();
        st.changes.push((1, "NewInstance".into(), "x".into())); // wird ignoriert
    }
    f.add_study(2, "aa-11", "s1");
    let mut p = poller(&e);
    assert_eq!(p.poll_once(), 1);
    assert_eq!((p.since, state_file(&e).as_str()), (2, "2"));
    assert_eq!(e.pipeline.pending_studies(), 1);
    assert_eq!(p.poll_once(), 0, "nichts Neues → nicht doppelt");
    assert_eq!(e.pipeline.pending_studies(), 1);
    // Neustart: Stand kommt aus der Datei
    let p2 = poller(&e);
    assert_eq!(p2.since, 2);
    // Archiviert wurde unter Patient/Studie
    let dcm: Vec<_> = fs::read_dir(e.cfg.archive_dir.join("4711"))
        .unwrap()
        .collect();
    assert_eq!(dcm.len(), 1);
}

#[test]
fn transient_failure_is_retried_then_succeeds() {
    let f = fake();
    let e = env(&url(&f), "");
    fs::create_dir_all(&e.cfg.archive_dir).unwrap();
    f.add_study(1, "aa-11", "s1");
    f.state.lock().unwrap().fail_files = true;
    let mut p = poller(&e);
    p.since = 0;
    assert_eq!(p.poll_once(), 0);
    assert_eq!(p.since, 0, "bei Fehler nicht weiterzählen");
    f.state.lock().unwrap().fail_files = false;
    assert_eq!(p.poll_once(), 1, "nächster Durchlauf klappt");
    assert_eq!(p.since, 1);
}

#[test]
fn deleted_study_is_skipped_and_later_ones_are_not_blocked() {
    let f = fake();
    let e = env(&url(&f), "");
    fs::create_dir_all(&e.cfg.archive_dir).unwrap();
    {
        let mut st = f.state.lock().unwrap();
        st.changes.push((1, "StableStudy".into(), "gone-0".into())); // Orthanc kennt sie nicht mehr → 404
        st.last = 1;
    }
    f.add_study(2, "aa-11", "s2");
    let mut p = poller(&e);
    p.since = 0;
    assert_eq!(
        p.poll_once(),
        1,
        "die gelöschte wird übersprungen, die zweite übernommen"
    );
    assert_eq!(p.since, 2);
}

#[test]
fn unsafe_ids_are_never_requested() {
    let f = fake();
    let e = env(&url(&f), "");
    fs::create_dir_all(&e.cfg.archive_dir).unwrap();
    {
        let mut st = f.state.lock().unwrap();
        st.changes
            .push((1, "StableStudy".into(), "../../etc".into()));
        st.last = 1;
    }
    let mut p = poller(&e);
    p.since = 0;
    assert_eq!(p.poll_once(), 0);
    assert_eq!(p.since, 1, "übersprungen");
    assert!(
        !f.state
            .lock()
            .unwrap()
            .requests
            .iter()
            .any(|(path, _)| path.contains("..")),
        "keine Anfrage mit .."
    );
}

#[test]
fn persistent_failure_is_skipped_after_max_attempts_and_later_studies_get_through() {
    let f = fake();
    let e = env(&url(&f), "");
    fs::create_dir_all(&e.cfg.archive_dir).unwrap();
    f.add_study(1, "aa-11", "s1");
    f.add_study(2, "bb-22", "s2"); // steht hinter der kaputten
    f.state
        .lock()
        .unwrap()
        .fail_instances
        .push("inst-s1".into());
    let mut p = poller(&e);
    p.since = 0;
    for attempt in 1..10 {
        assert_eq!(
            p.poll_once(),
            0,
            "Versuch {attempt}: blockiert, nichts übernommen"
        );
        assert_eq!(p.since, 0, "Versuch {attempt}: Stand bleibt");
    }
    // 10. Versuch: die kaputte wird übersprungen, die zweite kommt im selben Durchlauf durch
    assert_eq!(p.poll_once(), 1);
    assert_eq!(p.since, 2);
    assert_eq!(e.pipeline.pending_studies(), 1);
}

#[test]
fn invalid_dicom_file_is_skipped_without_blocking() {
    let f = fake();
    let e = env(&url(&f), "");
    fs::create_dir_all(&e.cfg.archive_dir).unwrap();
    {
        let mut st = f.state.lock().unwrap();
        st.changes.push((1, "StableStudy".into(), "aa-11".into()));
        st.studies
            .insert("aa-11".into(), vec!["inst-kaputt".into()]);
        st.files
            .insert("inst-kaputt".into(), b"kein dicom".to_vec());
        st.last = 1;
    }
    f.add_study(2, "bb-22", "s2");
    let mut p = poller(&e);
    p.since = 0;
    assert_eq!(
        p.poll_once(),
        2,
        "beide Untersuchungen gelten als bearbeitet"
    );
    assert_eq!(p.since, 2);
    assert_eq!(
        e.pipeline.pending_studies(),
        1,
        "nur die gültige liegt in der Pipeline"
    );
}

#[test]
fn unreachable_server_is_survived_and_recovered() {
    // Port reservieren, wieder freigeben → zunächst niemand da
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    drop(l);
    let e = env(&format!("http://{addr}"), "");
    let mut p = poller(&e); // Startpunkt kann nicht bestimmt werden, darf aber nicht scheitern
    assert_eq!(p.poll_once(), 0);
    assert_eq!(p.poll_once(), 0);
    // Server erscheint unter derselben Adresse
    let f = fake_at(TcpListener::bind(addr).unwrap());
    f.add_study(1, "aa-11", "s1");
    p.since = 0;
    assert_eq!(p.poll_once(), 1);
}

#[test]
fn credentials_are_sent_and_rejections_do_not_crash() {
    let f = fake();
    f.state.lock().unwrap().auth_required = Some("Basic dTpw".into()); // u:p
    f.add_study(1, "aa-11", "s1");
    // falsches Passwort
    let e = env(&url(&f), "orthanc_user = u\northanc_password = falsch");
    let mut p = poller(&e);
    p.since = 0;
    assert_eq!(p.poll_once(), 0);
    // richtiges Passwort
    let e2 = env(&url(&f), "orthanc_user = u\northanc_password = p");
    let mut p2 = poller(&e2);
    p2.since = 0;
    assert_eq!(p2.poll_once(), 1);
    assert!(f
        .state
        .lock()
        .unwrap()
        .requests
        .iter()
        .any(|(_, a)| a.as_deref() == Some("Basic dTpw")));
    // ohne Zugangsdaten
    let e3 = env(&url(&f), "");
    let mut p3 = poller(&e3);
    p3.since = 0;
    assert_eq!(p3.poll_once(), 0);
}

#[test]
fn orthanc_reset_is_detected() {
    let f = fake();
    f.state.lock().unwrap().last = 3;
    let e = env(&url(&f), "");
    fs::create_dir_all(&e.cfg.archive_dir).unwrap();
    fs::write(e.cfg.archive_dir.join(".orthanc-since"), "50").unwrap();
    let mut p = poller(&e);
    assert_eq!(p.since, 50);
    assert_eq!(p.poll_once(), 0);
    assert_eq!((p.since, state_file(&e).as_str()), (3, "3"));
    f.add_study(4, "aa-11", "s1");
    assert_eq!(p.poll_once(), 1, "danach läuft es wieder normal");
}

#[test]
fn paging_through_many_changes() {
    let f = fake();
    let e = env(&url(&f), "");
    fs::create_dir_all(&e.cfg.archive_dir).unwrap();
    {
        let mut st = f.state.lock().unwrap();
        for i in 1..=250u64 {
            st.changes.push((i, "NewInstance".into(), format!("i{i}")));
        }
        st.last = 250;
    }
    f.add_study(251, "aa-11", "s1");
    let mut p = poller(&e);
    p.since = 0;
    assert_eq!(p.poll_once(), 1);
    assert_eq!(p.since, 251);
    let n_change_requests = f
        .state
        .lock()
        .unwrap()
        .requests
        .iter()
        .filter(|(p, _)| p.starts_with("/changes?since="))
        .count();
    assert_eq!(n_change_requests, 3, "drei Seiten à 100");
}
