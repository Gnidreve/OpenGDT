//! Service Class Provider: nimmt Associations an und beantwortet
//! C-ECHO, C-FIND (Modality Worklist) und C-STORE.

use crate::assoc::*;
use crate::dataset::Dataset;
use crate::file::is_explicit;
use crate::pdu::{read_pdu, write_pdu, Pdu, PresentationContextAc};
use crate::tag::t;
use crate::{uid, Error};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct PeerInfo {
    pub calling_ae: String,
    pub called_ae: String,
    pub addr: String,
}

pub struct StoredObject {
    pub sop_class: String,
    pub sop_instance: String,
    pub transfer_syntax: String,
    /// Roh kodierter Datensatz in `transfer_syntax`.
    pub data: Vec<u8>,
}

/// Fachlogik hinter dem Server.
pub trait Handler: Send + Sync + 'static {
    fn supports_find(&self) -> bool {
        false
    }
    fn supports_store(&self) -> bool {
        false
    }
    /// Treffer für eine Worklist-Abfrage. `Err(status)` → Fehlerstatus.
    fn on_find(&self, _peer: &PeerInfo, _identifier: &Dataset) -> Result<Vec<Dataset>, u16> {
        Err(STATUS_UNRECOGNIZED_OPERATION)
    }
    /// Liefert den DIMSE-Status (0x0000 = Erfolg).
    fn on_store(&self, _peer: &PeerInfo, _obj: StoredObject) -> u16 {
        STATUS_UNRECOGNIZED_OPERATION
    }
    fn log(&self, msg: &str) {
        eprintln!("[dicom] {msg}");
    }
}

#[derive(Clone)]
pub struct ServerConfig {
    pub ae_title: String,
    /// `true`: Associations mit abweichendem Called AE werden abgelehnt.
    pub strict_called_ae: bool,
    pub read_timeout: Duration,
    pub max_connections: usize,
    /// Bevorzugte Transfer Syntaxes für Storage (UIDs, absteigende Priorität).
    /// Leer oder nichts davon vorgeschlagen: Reihenfolge der Gegenseite.
    pub storage_transfer_syntaxes: Vec<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
            ae_title: "OPENGDT".into(),
            strict_called_ae: false,
            read_timeout: Duration::from_secs(60),
            max_connections: 32,
            storage_transfer_syntaxes: Vec::new(),
        }
    }
}

pub struct Server {
    listener: TcpListener,
    cfg: ServerConfig,
    handler: Arc<dyn Handler>,
    stop: Arc<AtomicBool>,
}

impl Server {
    pub fn bind(addr: &str, cfg: ServerConfig, handler: Arc<dyn Handler>) -> Result<Server, Error> {
        let listener = TcpListener::bind(addr)?;
        listener.set_nonblocking(true)?;
        Ok(Server {
            listener,
            cfg,
            handler,
            stop: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.listener.local_addr().expect("lokale Adresse")
    }

    pub fn stop_flag(&self) -> Arc<AtomicBool> {
        self.stop.clone()
    }

    /// Blockiert, bis das Stop-Flag gesetzt wird.
    pub fn run(self) {
        let active = Arc::new(AtomicUsize::new(0));
        while !self.stop.load(Ordering::Relaxed) {
            match self.listener.accept() {
                Ok((stream, addr)) => {
                    if active.load(Ordering::Relaxed) >= self.cfg.max_connections {
                        self.handler.log(&format!(
                            "Verbindung von {addr} verworfen: zu viele Verbindungen"
                        ));
                        continue;
                    }
                    active.fetch_add(1, Ordering::Relaxed);
                    let cfg = self.cfg.clone();
                    let handler = self.handler.clone();
                    let active = active.clone();
                    std::thread::spawn(move || {
                        let _ = stream.set_nonblocking(false);
                        if let Err(e) =
                            serve_connection(stream, addr.to_string(), &cfg, handler.as_ref())
                        {
                            handler.log(&format!("Verbindung {addr}: {e}"));
                        }
                        active.fetch_sub(1, Ordering::Relaxed);
                    });
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(25));
                }
                Err(e) => {
                    self.handler.log(&format!("accept: {e}"));
                    std::thread::sleep(Duration::from_millis(250));
                }
            }
        }
    }
}

fn negotiate(
    contexts: &[crate::pdu::PresentationContextRq],
    handler: &dyn Handler,
    storage_preference: &[String],
) -> Vec<PresentationContextAc> {
    contexts
        .iter()
        .map(|c| {
            let a = c.abstract_syntax.as_str();
            let is_store = uid::is_storage_sop_class(a);
            let supported = a == uid::VERIFICATION
                || (a == uid::MWL_FIND && handler.supports_find())
                || (is_store && handler.supports_store());
            if !supported {
                return PresentationContextAc {
                    id: c.id,
                    result: 3,
                    transfer_syntax: uid::IMPLICIT_VR_LE.into(),
                };
            }
            let proposed = |u: &str| c.transfer_syntaxes.iter().any(|t| t == u);
            let chosen = if is_store {
                // Bevorzugte zuerst; sonst Reihenfolge der Gegenseite. Nie BE/Deflate.
                storage_preference
                    .iter()
                    .find(|p| proposed(p) && is_explicit(p).is_some())
                    .cloned()
                    .or_else(|| {
                        c.transfer_syntaxes
                            .iter()
                            .find(|t| is_explicit(t).is_some())
                            .cloned()
                    })
            } else if proposed(uid::EXPLICIT_VR_LE) {
                Some(uid::EXPLICIT_VR_LE.to_string())
            } else if proposed(uid::IMPLICIT_VR_LE) {
                Some(uid::IMPLICIT_VR_LE.to_string())
            } else {
                None
            };
            match chosen {
                Some(ts) => PresentationContextAc {
                    id: c.id,
                    result: 0,
                    transfer_syntax: ts,
                },
                None => PresentationContextAc {
                    id: c.id,
                    result: 4,
                    transfer_syntax: uid::IMPLICIT_VR_LE.into(),
                },
            }
        })
        .collect()
}

fn serve_connection(
    mut stream: TcpStream,
    addr: String,
    cfg: &ServerConfig,
    handler: &dyn Handler,
) -> Result<(), Error> {
    stream.set_read_timeout(Some(cfg.read_timeout))?;
    stream.set_write_timeout(Some(cfg.read_timeout))?;
    let (called, calling, contexts, peer_max) = match read_pdu(&mut stream)? {
        Pdu::AssociateRq {
            called_ae,
            calling_ae,
            contexts,
            max_pdu,
        } => (called_ae, calling_ae, contexts, max_pdu),
        _ => {
            write_pdu(
                &mut stream,
                &Pdu::Abort {
                    source: 2,
                    reason: 2,
                },
            )?;
            return Err(Error::Protocol("erstes PDU war kein A-ASSOCIATE-RQ".into()));
        }
    };
    if cfg.strict_called_ae && called != cfg.ae_title {
        // permanent / Service-User / Called AE Title nicht erkannt
        write_pdu(
            &mut stream,
            &Pdu::AssociateRj {
                result: 1,
                source: 1,
                reason: 7,
            },
        )?;
        return Err(Error::Rejected(format!(
            "Called AE '{called}' unbekannt (Peer {addr})"
        )));
    }
    let accepted = negotiate(&contexts, handler, &cfg.storage_transfer_syntaxes);
    write_pdu(
        &mut stream,
        &Pdu::AssociateAc {
            called_ae: called.clone(),
            calling_ae: calling.clone(),
            contexts: accepted.clone(),
            max_pdu: 131072,
        },
    )?;
    let mut assoc = Association::new(stream, accepted, peer_max, calling.clone(), called.clone());
    assoc.abstract_syntaxes = contexts
        .iter()
        .map(|c| (c.id, c.abstract_syntax.clone()))
        .collect();
    let peer = PeerInfo {
        calling_ae: calling,
        called_ae: called,
        addr,
    };

    while let Some(msg) = assoc.receive()? {
        handle_message(&mut assoc, &peer, handler, msg)?;
    }
    Ok(())
}

fn response(cmd_field: u16, request: &Dataset, status: u16) -> Dataset {
    let mut r = Dataset::new();
    if let Some(c) = request.get_str(t::AFFECTED_SOP_CLASS_UID) {
        r.set_str(t::AFFECTED_SOP_CLASS_UID, &c);
    }
    r.set_u16(t::COMMAND_FIELD, cmd_field);
    r.set_u16(
        t::MESSAGE_ID_RESPONDED_TO,
        request.get_u16(t::MESSAGE_ID).unwrap_or(0),
    );
    r.set_u16(t::STATUS, status);
    r
}

fn handle_message(
    assoc: &mut Association,
    peer: &PeerInfo,
    handler: &dyn Handler,
    msg: Message,
) -> Result<(), Error> {
    let ts = assoc
        .transfer_syntax(msg.context_id)
        .ok_or_else(|| Error::Protocol(format!("Context {} nicht akzeptiert", msg.context_id)))?
        .to_string();
    let explicit = is_explicit(&ts).ok_or(Error::Invalid("Transfer Syntax nicht unterstützt"))?;
    match msg.command_field() {
        CMD_C_ECHO_RQ => {
            let rsp = response(CMD_C_ECHO_RSP, &msg.command, STATUS_SUCCESS);
            assoc.send(msg.context_id, rsp, None)
        }
        CMD_C_FIND_RQ => {
            let data = msg
                .data
                .as_deref()
                .ok_or(Error::Invalid("C-FIND ohne Identifier"))?;
            let identifier = Dataset::decode(data, explicit)?;
            match handler.on_find(peer, &identifier) {
                Ok(matches) => {
                    for m in matches {
                        let rsp = response(CMD_C_FIND_RSP, &msg.command, STATUS_PENDING);
                        assoc.send(msg.context_id, rsp, Some(&m.encode(explicit)))?;
                    }
                    assoc.send(
                        msg.context_id,
                        response(CMD_C_FIND_RSP, &msg.command, STATUS_SUCCESS),
                        None,
                    )
                }
                Err(status) => assoc.send(
                    msg.context_id,
                    response(CMD_C_FIND_RSP, &msg.command, status),
                    None,
                ),
            }
        }
        CMD_C_STORE_RQ => {
            let data = msg.data.ok_or(Error::Invalid("C-STORE ohne Datensatz"))?;
            let sop_class = msg
                .command
                .get_str(t::AFFECTED_SOP_CLASS_UID)
                .unwrap_or_default();
            let sop_instance = msg
                .command
                .get_str(t::AFFECTED_SOP_INSTANCE_UID)
                .unwrap_or_default();
            let status = handler.on_store(
                peer,
                StoredObject {
                    sop_class,
                    sop_instance,
                    transfer_syntax: ts,
                    data,
                },
            );
            let mut rsp = response(CMD_C_STORE_RSP, &msg.command, status);
            if let Some(i) = msg.command.get_str(t::AFFECTED_SOP_INSTANCE_UID) {
                rsp.set_str(t::AFFECTED_SOP_INSTANCE_UID, &i);
            }
            assoc.send(msg.context_id, rsp, None)
        }
        other => {
            handler.log(&format!(
                "Kommando {other:#06x} von {} nicht unterstützt",
                peer.calling_ae
            ));
            write_pdu(
                &mut assoc.stream,
                &Pdu::Abort {
                    source: 2,
                    reason: 2,
                },
            )?;
            Err(Error::Protocol(format!(
                "Kommando {other:#06x} nicht unterstützt"
            )))
        }
    }
}
