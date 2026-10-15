//! Rust-SCU gegen Rust-SCP über Loopback.

use opengdt_dicom::scp::{Handler, PeerInfo, Server, ServerConfig, StoredObject};
use opengdt_dicom::scu::{connect, ScuConfig};
use opengdt_dicom::{t, uid, Dataset};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Default)]
struct Rec {
    stored: Mutex<Vec<(String, String, usize)>>,
}

impl Handler for Rec {
    fn supports_find(&self) -> bool {
        true
    }
    fn supports_store(&self) -> bool {
        true
    }
    fn on_find(&self, _p: &PeerInfo, id: &Dataset) -> Result<Vec<Dataset>, u16> {
        let mut m = Dataset::new();
        m.set_str(t::PATIENT_NAME, "Müller^Jürgen");
        m.set_str(
            t::PATIENT_ID,
            id.get_str(t::PATIENT_ID).unwrap_or_default().as_str(),
        );
        Ok(vec![m.clone(), m])
    }
    fn on_store(&self, _p: &PeerInfo, o: StoredObject) -> u16 {
        self.stored
            .lock()
            .unwrap()
            .push((o.sop_class, o.sop_instance, o.data.len()));
        0
    }
    fn log(&self, _msg: &str) {}
}

fn start(
    h: Arc<Rec>,
    cfg: ServerConfig,
) -> (std::net::SocketAddr, Arc<std::sync::atomic::AtomicBool>) {
    let server = Server::bind("127.0.0.1:0", cfg, h).unwrap();
    let addr = server.local_addr();
    let stop = server.stop_flag();
    std::thread::spawn(move || server.run());
    (addr, stop)
}

fn scu_cfg() -> ScuConfig {
    ScuConfig {
        calling_ae: "TEST".into(),
        called_ae: "OPENGDT".into(),
        timeout: Duration::from_secs(5),
    }
}

#[test]
fn echo_find_store() {
    let h = Arc::new(Rec::default());
    let (addr, stop) = start(h.clone(), ServerConfig::default());
    let ts: &[&str] = &[uid::EXPLICIT_VR_LE, uid::IMPLICIT_VR_LE];
    let mut a = connect(
        addr,
        &scu_cfg(),
        &[
            (uid::VERIFICATION, ts),
            (uid::MWL_FIND, ts),
            ("1.2.840.10008.5.1.4.1.1.6.1", ts),
        ],
    )
    .unwrap();
    assert_eq!(a.echo().unwrap(), 0);

    let mut q = Dataset::new();
    q.set_str(t::PATIENT_ID, "4711");
    let (status, matches) = a.find(&q).unwrap();
    assert_eq!(status, 0);
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].get_str(t::PATIENT_ID).as_deref(), Some("4711"));
    assert_eq!(
        matches[0].get_str(t::PATIENT_NAME).as_deref(),
        Some("Müller^Jürgen")
    );

    // Große Nutzlast → muss in mehrere PDUs fragmentiert werden.
    let payload = vec![7u8; 300_000];
    assert_eq!(
        a.store("1.2.840.10008.5.1.4.1.1.6.1", "1.2.3.4.5", &payload)
            .unwrap(),
        0
    );
    a.release().unwrap();

    let stored = h.stored.lock().unwrap();
    assert_eq!(
        stored.as_slice(),
        &[(
            "1.2.840.10008.5.1.4.1.1.6.1".to_string(),
            "1.2.3.4.5".to_string(),
            300_000
        )]
    );
    stop.store(true, Ordering::Relaxed);
}

#[test]
fn strict_called_ae_rejects() {
    let h = Arc::new(Rec::default());
    let cfg = ServerConfig {
        strict_called_ae: true,
        ..ServerConfig::default()
    };
    let (addr, stop) = start(h, cfg);
    let mut c = scu_cfg();
    c.called_ae = "WRONG".into();
    let r = connect(addr, &c, &[(uid::VERIFICATION, &[uid::IMPLICIT_VR_LE])]);
    assert!(matches!(r, Err(opengdt_dicom::Error::Rejected(_))));
    stop.store(true, Ordering::Relaxed);
}

#[test]
fn unsupported_abstract_syntax_is_not_accepted() {
    let h = Arc::new(Rec::default());
    let (addr, stop) = start(h, ServerConfig::default());
    let a = connect(
        addr,
        &scu_cfg(),
        &[
            (uid::VERIFICATION, &[uid::IMPLICIT_VR_LE]),
            ("1.2.3.999", &[uid::IMPLICIT_VR_LE]),
        ],
    )
    .unwrap();
    assert!(a.context_for("1.2.3.999").is_none());
    assert!(a.context_for(uid::VERIFICATION).is_some());
    a.release().unwrap();
    stop.store(true, Ordering::Relaxed);
}

#[test]
fn garbage_does_not_kill_server() {
    use std::io::Write;
    let h = Arc::new(Rec::default());
    let (addr, stop) = start(h, ServerConfig::default());
    let mut s = std::net::TcpStream::connect(addr).unwrap();
    s.write_all(&[0xFF; 64]).unwrap();
    drop(s);
    let mut a = connect(
        addr,
        &scu_cfg(),
        &[(uid::VERIFICATION, &[uid::IMPLICIT_VR_LE])],
    )
    .unwrap();
    assert_eq!(a.echo().unwrap(), 0);
    stop.store(true, Ordering::Relaxed);
}
