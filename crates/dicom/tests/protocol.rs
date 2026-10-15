//! Fehler- und Randfälle des Protokolls: abgelehnte Contexts, kaputte Gegenstellen, Limits.

use opengdt_dicom::assoc::*;
use opengdt_dicom::pdu::{read_pdu, write_pdu, Pdu, Pdv, PresentationContextAc};
use opengdt_dicom::scp::{Handler, PeerInfo, Server, ServerConfig, StoredObject};
use opengdt_dicom::scu::{connect, ScuConfig};
use opengdt_dicom::{t, uid, Dataset, Error};
use std::io::Read;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const US: &str = "1.2.840.10008.5.1.4.1.1.6.1";

/// Konfigurierbarer Handler, der alles mitschreibt.
struct Script {
    logs: Mutex<Vec<String>>,
    find: Mutex<Result<Vec<Dataset>, u16>>,
    store_status: u16,
    stored: Mutex<Vec<usize>>,
}

impl Script {
    fn new() -> Arc<Script> {
        Arc::new(Script {
            logs: Mutex::new(vec![]),
            find: Mutex::new(Ok(vec![])),
            store_status: 0,
            stored: Mutex::new(vec![]),
        })
    }
}

impl Handler for Script {
    fn supports_find(&self) -> bool {
        true
    }
    fn supports_store(&self) -> bool {
        true
    }
    fn on_find(&self, _p: &PeerInfo, _i: &Dataset) -> Result<Vec<Dataset>, u16> {
        self.find.lock().unwrap().clone()
    }
    fn on_store(&self, _p: &PeerInfo, o: StoredObject) -> u16 {
        self.stored.lock().unwrap().push(o.data.len());
        self.store_status
    }
    fn log(&self, msg: &str) {
        self.logs.lock().unwrap().push(msg.to_string());
    }
}

/// Handler, der nur die Standardimplementierungen des Traits benutzt.
struct DefaultsOnly;
impl Handler for DefaultsOnly {}

fn start(h: Arc<dyn Handler>, cfg: ServerConfig) -> (SocketAddr, Arc<AtomicBool>) {
    let server = Server::bind("127.0.0.1:0", cfg, h).unwrap();
    let addr = server.local_addr();
    let stop = server.stop_flag();
    std::thread::spawn(move || server.run());
    (addr, stop)
}

fn scu() -> ScuConfig {
    ScuConfig {
        calling_ae: "T".into(),
        called_ae: "OPENGDT".into(),
        timeout: Duration::from_secs(5),
    }
}

const IMPLICIT: &[&str] = &[uid::IMPLICIT_VR_LE];

fn echo_ok(addr: SocketAddr) -> bool {
    connect(addr, &scu(), &[(uid::VERIFICATION, IMPLICIT)])
        .and_then(|mut a| a.echo())
        .map(|s| s == 0)
        .unwrap_or(false)
}

fn wait_until(what: &str, mut f: impl FnMut() -> bool) {
    let start = Instant::now();
    while !f() {
        assert!(start.elapsed() < Duration::from_secs(5), "Timeout: {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

// ------------------------------------------------------------------ SCP: Defaults

#[test]
fn trait_defaults_refuse_everything_but_echo() {
    let h = DefaultsOnly;
    let peer = PeerInfo {
        calling_ae: "X".into(),
        called_ae: "Y".into(),
        addr: "z".into(),
    };
    assert!(!h.supports_find() && !h.supports_store());
    assert_eq!(
        h.on_find(&peer, &Dataset::new()),
        Err(STATUS_UNRECOGNIZED_OPERATION)
    );
    let obj = StoredObject {
        sop_class: US.into(),
        sop_instance: "1.2".into(),
        transfer_syntax: uid::EXPLICIT_VR_LE.into(),
        data: vec![],
    };
    assert_eq!(h.on_store(&peer, obj), STATUS_UNRECOGNIZED_OPERATION);
    h.log("Standard-Log geht auf stderr");

    // Über das Netzwerk: nur Verification wird angenommen
    let (addr, stop) = start(Arc::new(DefaultsOnly), ServerConfig::default());
    let a = connect(
        addr,
        &scu(),
        &[
            (uid::VERIFICATION, IMPLICIT),
            (uid::MWL_FIND, IMPLICIT),
            (US, IMPLICIT),
        ],
    )
    .unwrap();
    assert!(a.context_for(uid::VERIFICATION).is_some());
    assert!(a.context_for(uid::MWL_FIND).is_none());
    assert!(a.context_for(US).is_none());
    a.release().unwrap();
    stop.store(true, Ordering::Relaxed);
}

// ------------------------------------------------------------ SCP: Aushandlung

#[test]
fn contexts_without_usable_transfer_syntax_are_rejected_with_reason_4() {
    let (addr, stop) = start(Script::new(), ServerConfig::default());
    let be: &[&str] = &[uid::EXPLICIT_VR_BE];
    let deflate: &[&str] = &[uid::DEFLATED_EXPLICIT_VR_LE];
    let a = connect(
        addr,
        &scu(),
        &[
            (uid::MWL_FIND, be),
            (US, deflate),
            (uid::VERIFICATION, IMPLICIT),
        ],
    )
    .unwrap();
    assert!(
        a.context_for(uid::MWL_FIND).is_none(),
        "Worklist nur Big Endian"
    );
    assert!(a.context_for(US).is_none(), "Storage nur Deflate");
    assert!(a.context_for(uid::VERIFICATION).is_some());
    assert!(a.contexts.iter().filter(|c| c.result == 4).count() == 2);
    a.release().unwrap();
    stop.store(true, Ordering::Relaxed);
}

#[test]
fn worklist_prefers_explicit_over_implicit_and_falls_back() {
    let (addr, stop) = start(Script::new(), ServerConfig::default());
    let both: &[&str] = &[uid::IMPLICIT_VR_LE, uid::EXPLICIT_VR_LE];
    let a = connect(addr, &scu(), &[(uid::MWL_FIND, both)]).unwrap();
    assert_eq!(
        a.context_for(uid::MWL_FIND).map(|c| c.1),
        Some(uid::EXPLICIT_VR_LE)
    );
    a.release().unwrap();
    let a = connect(addr, &scu(), &[(uid::MWL_FIND, IMPLICIT)]).unwrap();
    assert_eq!(
        a.context_for(uid::MWL_FIND).map(|c| c.1),
        Some(uid::IMPLICIT_VR_LE)
    );
    a.release().unwrap();
    stop.store(true, Ordering::Relaxed);
}

#[test]
fn storage_preference_list_is_honoured() {
    let cfg = ServerConfig {
        storage_transfer_syntaxes: vec![uid::JPEG_BASELINE.into(), uid::EXPLICIT_VR_LE.into()],
        ..ServerConfig::default()
    };
    let (addr, stop) = start(Script::new(), cfg);
    let offered: &[&str] = &[uid::IMPLICIT_VR_LE, uid::EXPLICIT_VR_LE, uid::JPEG_BASELINE];
    let a = connect(addr, &scu(), &[(US, offered)]).unwrap();
    assert_eq!(a.context_for(US).map(|c| c.1), Some(uid::JPEG_BASELINE));
    a.release().unwrap();
    // BE/Deflate in der Präferenzliste würden nie gewählt, auch wenn angeboten
    let cfg = ServerConfig {
        storage_transfer_syntaxes: vec![uid::EXPLICIT_VR_BE.into()],
        ..ServerConfig::default()
    };
    let (addr2, stop2) = start(Script::new(), cfg);
    let offered: &[&str] = &[uid::EXPLICIT_VR_BE, uid::IMPLICIT_VR_LE];
    let a = connect(addr2, &scu(), &[(US, offered)]).unwrap();
    assert_eq!(a.context_for(US).map(|c| c.1), Some(uid::IMPLICIT_VR_LE));
    a.release().unwrap();
    stop.store(true, Ordering::Relaxed);
    stop2.store(true, Ordering::Relaxed);
}

// ----------------------------------------------------------- SCP: Fehlverhalten

#[test]
fn first_pdu_other_than_associate_gets_abort() {
    let h = Script::new();
    let (addr, stop) = start(h.clone(), ServerConfig::default());
    let mut s = TcpStream::connect(addr).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    write_pdu(&mut s, &Pdu::ReleaseRq).unwrap();
    assert_eq!(
        read_pdu(&mut s).unwrap(),
        Pdu::Abort {
            source: 2,
            reason: 2
        }
    );
    wait_until("Log-Eintrag", || !h.logs.lock().unwrap().is_empty());
    assert!(h.logs.lock().unwrap()[0].contains("A-ASSOCIATE-RQ"));
    assert!(echo_ok(addr), "Server läuft weiter");
    stop.store(true, Ordering::Relaxed);
}

fn echo_command() -> Dataset {
    let mut c = Dataset::new();
    c.set_str(t::AFFECTED_SOP_CLASS_UID, uid::VERIFICATION);
    c.set_u16(t::COMMAND_FIELD, CMD_C_ECHO_RQ);
    c.set_u16(t::MESSAGE_ID, 1);
    c
}

#[test]
fn unsupported_command_aborts_and_is_logged() {
    let h = Script::new();
    let (addr, stop) = start(h.clone(), ServerConfig::default());
    let mut a = connect(addr, &scu(), &[(uid::VERIFICATION, IMPLICIT)]).unwrap();
    let (ctx, _) = a.context_for(uid::VERIFICATION).unwrap();
    let mut c = echo_command();
    c.set_u16(t::COMMAND_FIELD, 0x0010); // C-GET-RQ: nicht unterstützt
    a.send(ctx, c, None).unwrap();
    // Gegenseite bricht ab → receive() meldet das Ende
    assert!(matches!(a.receive(), Ok(None) | Err(_)));
    wait_until("Log", || {
        h.logs
            .lock()
            .unwrap()
            .iter()
            .any(|l| l.contains("nicht unterstützt"))
    });
    assert!(echo_ok(addr));
    stop.store(true, Ordering::Relaxed);
}

#[test]
fn message_on_rejected_context_closes_only_that_connection() {
    let (addr, stop) = start(Script::new(), ServerConfig::default());
    let mut a = connect(
        addr,
        &scu(),
        &[(uid::VERIFICATION, IMPLICIT), ("1.2.3.999", IMPLICIT)],
    )
    .unwrap();
    // Context 3 wurde abgelehnt, wir senden trotzdem darauf
    a.send(3, echo_command(), None).unwrap();
    assert!(matches!(a.receive(), Ok(None) | Err(_)));
    assert!(echo_ok(addr));
    stop.store(true, Ordering::Relaxed);
}

#[test]
fn store_without_dataset_and_find_with_garbage_identifier_are_survived() {
    let h = Script::new();
    let (addr, stop) = start(h.clone(), ServerConfig::default());
    let ex: &[&str] = &[uid::EXPLICIT_VR_LE];
    let mut a = connect(addr, &scu(), &[(US, ex), (uid::MWL_FIND, ex)]).unwrap();
    let (ctx, _) = a.context_for(US).unwrap();
    let mut c = echo_command();
    c.set_str(t::AFFECTED_SOP_CLASS_UID, US);
    c.set_u16(t::COMMAND_FIELD, CMD_C_STORE_RQ);
    c.set_str(t::AFFECTED_SOP_INSTANCE_UID, "1.2.3");
    a.send(ctx, c, None).unwrap(); // Command ohne Datensatz
    assert!(matches!(a.receive(), Ok(None) | Err(_)));
    assert!(h.stored.lock().unwrap().is_empty(), "nichts gespeichert");

    let mut a = connect(addr, &scu(), &[(uid::MWL_FIND, ex)]).unwrap();
    let (ctx, _) = a.context_for(uid::MWL_FIND).unwrap();
    let mut c = echo_command();
    c.set_str(t::AFFECTED_SOP_CLASS_UID, uid::MWL_FIND);
    c.set_u16(t::COMMAND_FIELD, CMD_C_FIND_RQ);
    a.send(
        ctx,
        c,
        Some(&[0x10, 0x00, 0x10, 0x00, b'P', b'N', 0xFF, 0xFF]),
    )
    .unwrap();
    assert!(matches!(a.receive(), Ok(None) | Err(_)));
    assert!(echo_ok(addr));
    stop.store(true, Ordering::Relaxed);
}

#[test]
fn find_failure_status_is_forwarded() {
    let h = Script::new();
    *h.find.lock().unwrap() = Err(STATUS_REFUSED_OUT_OF_RESOURCES);
    let (addr, stop) = start(h.clone(), ServerConfig::default());
    let mut a = connect(addr, &scu(), &[(uid::MWL_FIND, IMPLICIT)]).unwrap();
    let (status, matches) = a.find(&Dataset::new()).unwrap();
    assert_eq!(
        (status, matches.len()),
        (STATUS_REFUSED_OUT_OF_RESOURCES, 0)
    );
    a.release().unwrap();
    stop.store(true, Ordering::Relaxed);
}

#[test]
fn store_status_and_empty_dataset_are_passed_through() {
    let h = Arc::new(Script {
        logs: Mutex::new(vec![]),
        find: Mutex::new(Ok(vec![])),
        store_status: STATUS_REFUSED_OUT_OF_RESOURCES,
        stored: Mutex::new(vec![]),
    });
    let (addr, stop) = start(h.clone(), ServerConfig::default());
    let mut a = connect(addr, &scu(), &[(US, IMPLICIT)]).unwrap();
    // leerer Datensatz → ein einziges, leeres Daten-PDV
    assert_eq!(
        a.store(US, "1.2.3", &[]).unwrap(),
        STATUS_REFUSED_OUT_OF_RESOURCES
    );
    assert_eq!(h.stored.lock().unwrap().as_slice(), &[0]);
    a.release().unwrap();
    stop.store(true, Ordering::Relaxed);
}

#[test]
fn protocol_violations_inside_association_end_it_cleanly() {
    let h = Script::new();
    let (addr, stop) = start(h.clone(), ServerConfig::default());

    // 1. Daten-PDV ohne vorheriges Command
    let mut a = connect(addr, &scu(), &[(uid::VERIFICATION, IMPLICIT)]).unwrap();
    let pdv = Pdv {
        context_id: 1,
        is_command: false,
        is_last: true,
        data: vec![1, 2, 3],
    };
    write_pdu(&mut a.stream, &Pdu::Data(vec![pdv])).unwrap();
    assert!(matches!(a.receive(), Ok(None) | Err(_)));
    wait_until("Log 1", || {
        h.logs
            .lock()
            .unwrap()
            .iter()
            .any(|l| l.contains("Daten ohne Command"))
    });

    // 2. Zweites A-ASSOCIATE-RQ mitten in der Association
    let mut a = connect(addr, &scu(), &[(uid::VERIFICATION, IMPLICIT)]).unwrap();
    write_pdu(
        &mut a.stream,
        &Pdu::AssociateRq {
            called_ae: "A".into(),
            calling_ae: "B".into(),
            contexts: vec![],
            max_pdu: 16384,
        },
    )
    .unwrap();
    assert!(matches!(a.receive(), Ok(None) | Err(_)));
    wait_until("Log 2", || {
        h.logs
            .lock()
            .unwrap()
            .iter()
            .any(|l| l.contains("unerwartetes PDU"))
    });

    // 3. Command-Bytes, die kein gültiger Datensatz sind
    let mut a = connect(addr, &scu(), &[(uid::VERIFICATION, IMPLICIT)]).unwrap();
    let pdv = Pdv {
        context_id: 1,
        is_command: true,
        is_last: true,
        data: vec![0x00, 0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0x7F],
    };
    write_pdu(&mut a.stream, &Pdu::Data(vec![pdv])).unwrap();
    assert!(matches!(a.receive(), Ok(None) | Err(_)));
    // 4. PDU mit absurder Länge (Speicherbomben-Schutz) mitten in der Association
    let mut a = connect(addr, &scu(), &[(uid::VERIFICATION, IMPLICIT)]).unwrap();
    std::io::Write::write_all(&mut a.stream, &[0x04, 0x00, 0xFF, 0xFF, 0xFF, 0xFF]).unwrap();
    assert!(matches!(a.receive(), Ok(None) | Err(_)));
    wait_until("Log 4", || {
        h.logs.lock().unwrap().iter().any(|l| l.contains("Limit"))
    });
    assert!(echo_ok(addr), "Server überlebt alle vier");
    stop.store(true, Ordering::Relaxed);
}

// ------------------------------------------------------------- SCP: Limits/Timeout

#[test]
fn connection_limit_drops_extra_connections_until_a_slot_is_free() {
    let h = Script::new();
    let cfg = ServerConfig {
        max_connections: 1,
        ..ServerConfig::default()
    };
    let (addr, stop) = start(h.clone(), cfg);
    let first = connect(addr, &scu(), &[(uid::VERIFICATION, IMPLICIT)]).unwrap();
    assert!(
        connect(addr, &scu(), &[(uid::VERIFICATION, IMPLICIT)]).is_err(),
        "zweite Verbindung muss verworfen werden"
    );
    wait_until("Log", || {
        h.logs
            .lock()
            .unwrap()
            .iter()
            .any(|l| l.contains("zu viele"))
    });
    first.release().unwrap();
    wait_until("Platz frei", || echo_ok(addr));
    stop.store(true, Ordering::Relaxed);
}

#[test]
fn idle_connection_is_closed_after_read_timeout() {
    let cfg = ServerConfig {
        read_timeout: Duration::from_millis(200),
        ..ServerConfig::default()
    };
    let (addr, stop) = start(Script::new(), cfg);
    let mut s = TcpStream::connect(addr).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let mut buf = [0u8; 1];
    let start = Instant::now();
    assert_eq!(
        s.read(&mut buf).unwrap(),
        0,
        "Server schließt die untätige Verbindung"
    );
    assert!(start.elapsed() < Duration::from_secs(3));
    stop.store(true, Ordering::Relaxed);
}

#[test]
fn run_returns_after_stop_and_bind_errors_are_reported() {
    let server = Server::bind("127.0.0.1:0", ServerConfig::default(), Script::new()).unwrap();
    let addr = server.local_addr();
    let stop = server.stop_flag();
    let handle = std::thread::spawn(move || server.run());
    // Derselbe Port ist belegt
    assert!(matches!(
        Server::bind(&addr.to_string(), ServerConfig::default(), Script::new()),
        Err(Error::Io(_))
    ));
    assert!(Server::bind("kein-host:xyz", ServerConfig::default(), Script::new()).is_err());
    stop.store(true, Ordering::Relaxed);
    let start = Instant::now();
    while !handle.is_finished() {
        assert!(
            start.elapsed() < Duration::from_secs(3),
            "run() kehrt nicht zurück"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    handle.join().unwrap();
}

// --------------------------------------------------------------- SCU gegen Fake-SCP

/// Fake-Gegenstelle: nimmt die Association an (alle Contexts, erste Transfer Syntax)
/// und übergibt sie dem Skript.
fn fake_scp<F: FnOnce(Association) + Send + 'static>(script: F) -> SocketAddr {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    std::thread::spawn(move || {
        let (mut s, _) = l.accept().unwrap();
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let Pdu::AssociateRq {
            called_ae,
            calling_ae,
            contexts,
            ..
        } = read_pdu(&mut s).unwrap()
        else {
            panic!("kein A-ASSOCIATE-RQ")
        };
        let ac: Vec<PresentationContextAc> = contexts
            .iter()
            .map(|c| PresentationContextAc {
                id: c.id,
                result: 0,
                transfer_syntax: c.transfer_syntaxes[0].clone(),
            })
            .collect();
        write_pdu(
            &mut s,
            &Pdu::AssociateAc {
                called_ae: called_ae.clone(),
                calling_ae: calling_ae.clone(),
                contexts: ac.clone(),
                max_pdu: 16384,
            },
        )
        .unwrap();
        let mut a = Association::new(s, ac, 16384, calling_ae, called_ae);
        a.abstract_syntaxes = contexts
            .iter()
            .map(|c| (c.id, c.abstract_syntax.clone()))
            .collect();
        script(a);
    });
    addr
}

fn rsp(cmd: u16, status: u16) -> Dataset {
    let mut r = Dataset::new();
    r.set_u16(t::COMMAND_FIELD, cmd);
    r.set_u16(t::MESSAGE_ID_RESPONDED_TO, 1);
    r.set_u16(t::STATUS, status);
    r
}

#[test]
fn scu_find_accepts_both_pending_codes_and_returns_final_status() {
    let addr = fake_scp(|mut a| {
        let msg = a.receive().unwrap().unwrap();
        let mut m = Dataset::new();
        m.set_str(t::PATIENT_ID, "1");
        let data = m.encode(false);
        a.send(
            msg.context_id,
            rsp(CMD_C_FIND_RSP, STATUS_PENDING),
            Some(&data),
        )
        .unwrap();
        a.send(msg.context_id, rsp(CMD_C_FIND_RSP, 0xFF01), Some(&data))
            .unwrap();
        a.send(msg.context_id, rsp(CMD_C_FIND_RSP, 0xFE00), None)
            .unwrap(); // Cancel
        let _ = a.receive();
    });
    let mut a = connect(addr, &scu(), &[(uid::MWL_FIND, IMPLICIT)]).unwrap();
    let (status, matches) = a.find(&Dataset::new()).unwrap();
    assert_eq!(status, 0xFE00);
    assert_eq!(matches.len(), 2);
}

#[test]
fn scu_find_reports_connection_lost_midway() {
    let addr = fake_scp(|mut a| {
        let _ = a.receive(); // Anfrage lesen, dann einfach auflegen
    });
    let mut a = connect(addr, &scu(), &[(uid::MWL_FIND, IMPLICIT)]).unwrap();
    assert!(matches!(
        a.find(&Dataset::new()),
        Err(Error::Protocol(_)) | Err(Error::Io(_))
    ));
}

#[test]
fn scu_echo_and_store_return_the_peers_status() {
    let addr = fake_scp(|mut a| {
        let m = a.receive().unwrap().unwrap();
        a.send(m.context_id, rsp(CMD_C_ECHO_RSP, 0x0110), None)
            .unwrap();
        let m = a.receive().unwrap().unwrap();
        a.send(m.context_id, rsp(CMD_C_STORE_RSP, 0xB000), None)
            .unwrap();
        let _ = a.receive();
    });
    let mut a = connect(
        addr,
        &scu(),
        &[(uid::VERIFICATION, IMPLICIT), (US, IMPLICIT)],
    )
    .unwrap();
    assert_eq!(a.echo().unwrap(), 0x0110);
    assert_eq!(a.store(US, "1.2.3", &[1, 2]).unwrap(), 0xB000);
}

#[test]
fn scu_operations_on_unaccepted_abstract_syntax_fail_cleanly() {
    let (addr, stop) = start(Script::new(), ServerConfig::default());
    let mut a = connect(addr, &scu(), &[("1.2.3.999", IMPLICIT), (US, IMPLICIT)]).unwrap();
    // Verification wurde nie vorgeschlagen, MWL ebenfalls nicht
    assert!(matches!(a.echo(), Err(Error::Rejected(_))));
    assert!(matches!(a.find(&Dataset::new()), Err(Error::Rejected(_))));
    assert!(matches!(
        a.store("1.2.3.999", "1.2", &[]),
        Err(Error::Rejected(_))
    ));
    stop.store(true, Ordering::Relaxed);
}

#[test]
fn scu_connect_rejects_unexpected_first_answer() {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    std::thread::spawn(move || {
        let (mut s, _) = l.accept().unwrap();
        let _ = read_pdu(&mut s);
        write_pdu(&mut s, &Pdu::ReleaseRp).unwrap();
    });
    assert!(matches!(
        connect(addr, &scu(), &[(uid::VERIFICATION, IMPLICIT)]),
        Err(Error::Protocol(_))
    ));
}

#[test]
fn scu_release_detects_unexpected_answer() {
    let addr = fake_scp(|mut a| {
        let _ = read_pdu(&mut a.stream); // A-RELEASE-RQ
        write_pdu(
            &mut a.stream,
            &Pdu::Abort {
                source: 2,
                reason: 0,
            },
        )
        .unwrap();
    });
    let a = connect(addr, &scu(), &[(uid::VERIFICATION, IMPLICIT)]).unwrap();
    assert!(matches!(a.release(), Err(Error::Protocol(_))));
}

#[test]
fn scu_connect_to_closed_port_is_io_error() {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    drop(l);
    assert!(matches!(
        connect(addr, &scu(), &[(uid::VERIFICATION, IMPLICIT)]),
        Err(Error::Io(_))
    ));
}

#[test]
fn error_display_texts() {
    assert!(Error::Truncated.to_string().contains("abgeschnitten"));
    assert!(Error::Invalid("x").to_string().contains("ungültig"));
    assert!(Error::Protocol("p".into())
        .to_string()
        .contains("Protokoll"));
    assert!(Error::Rejected("r".into())
        .to_string()
        .contains("abgelehnt"));
    assert!(Error::Io(std::io::Error::other("kaputt"))
        .to_string()
        .contains("kaputt"));
}

// ------------------------------------------------------------ Speichergrenzen

#[test]
fn oversized_messages_end_the_association_instead_of_filling_memory() {
    let (tx, rx) = std::sync::mpsc::channel();
    let addr = fake_scp(move |mut a| {
        a.max_message_bytes = 10_000;
        let r = a.receive();
        let _ = tx.send(match r {
            Err(Error::Protocol(m)) => m,
            other => format!("unerwartet: {other:?}"),
        });
    });
    let mut a = connect(addr, &scu(), &[(US, IMPLICIT)]).unwrap();
    // 50 000 Bytes Datensatz > Limit 10 000
    let _ = a.store(US, "1.2.3", &vec![7u8; 50_000]);
    let msg = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(msg.contains("Größenlimit"), "{msg}");
}

#[test]
fn pdu_size_limit_is_small_enough_for_a_pi() {
    // Ein PDU mit 8 MiB Länge im Kopf wird abgelehnt, ohne dass der Server so viel Speicher anlegt.
    let (addr, stop) = start(Script::new(), ServerConfig::default());
    let mut s = TcpStream::connect(addr).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    std::io::Write::write_all(&mut s, &[0x01, 0x00, 0x00, 0x80, 0x00, 0x00]).unwrap(); // A-ASSOCIATE-RQ, 8 MiB
    let mut buf = [0u8; 8];
    assert_eq!(
        s.read(&mut buf).unwrap(),
        0,
        "Server schließt die Verbindung"
    );
    assert!(echo_ok(addr));
    stop.store(true, Ordering::Relaxed);
}
