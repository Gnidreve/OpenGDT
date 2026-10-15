//! Association: Senden/Empfangen von DIMSE-Nachrichten über P-DATA-TF.

use crate::dataset::{command_group_length, Dataset};
use crate::pdu::{read_pdu, write_pdu, Pdu, Pdv, PresentationContextAc};
use crate::tag::t;
use crate::Error;
use std::io::{Read, Write};
use std::net::TcpStream;

pub const CMD_C_STORE_RQ: u16 = 0x0001;
pub const CMD_C_STORE_RSP: u16 = 0x8001;
pub const CMD_C_FIND_RQ: u16 = 0x0020;
pub const CMD_C_FIND_RSP: u16 = 0x8020;
pub const CMD_C_ECHO_RQ: u16 = 0x0030;
pub const CMD_C_ECHO_RSP: u16 = 0x8030;

pub const NO_DATASET: u16 = 0x0101;
pub const STATUS_SUCCESS: u16 = 0x0000;
pub const STATUS_PENDING: u16 = 0xFF00;
pub const STATUS_REFUSED_OUT_OF_RESOURCES: u16 = 0xA700;
pub const STATUS_PROCESSING_FAILURE: u16 = 0x0110;
pub const STATUS_UNRECOGNIZED_OPERATION: u16 = 0x0211;
pub const STATUS_IDENTIFIER_MISMATCH: u16 = 0xA900;

/// Eine vollständige DIMSE-Nachricht.
#[derive(Debug)]
pub struct Message {
    pub context_id: u8,
    pub command: Dataset,
    /// Roh-Datensatz in der Transfer Syntax des Presentation Context.
    pub data: Option<Vec<u8>>,
}

impl Message {
    pub fn command_field(&self) -> u16 {
        self.command.get_u16(t::COMMAND_FIELD).unwrap_or(0)
    }
}

pub struct Association<S: Read + Write = TcpStream> {
    pub stream: S,
    pub contexts: Vec<PresentationContextAc>,
    /// Maximale PDU-Größe der Gegenseite (Datenteil).
    pub peer_max_pdu: u32,
    pub calling_ae: String,
    pub called_ae: String,
    /// (Context-ID, Abstract Syntax) der vorgeschlagenen/verhandelten Contexts.
    pub abstract_syntaxes: Vec<(u8, String)>,
    /// Obergrenze für eine einzelne empfangene Nachricht (Command oder Datensatz) in Bytes. Größere
    /// Nachrichten beenden die Association – sonst könnte ein Client den Speicher füllen.
    pub max_message_bytes: usize,
    next_message_id: u16,
}

/// Standardlimit: großzügig für Cine-Clips, aber unterhalb des Speichers eines kleinen Pi.
pub const DEFAULT_MAX_MESSAGE_BYTES: usize = 512 * 1024 * 1024;

impl<S: Read + Write> Association<S> {
    pub fn new(
        stream: S,
        contexts: Vec<PresentationContextAc>,
        peer_max_pdu: u32,
        calling_ae: String,
        called_ae: String,
    ) -> Self {
        Association {
            stream,
            contexts,
            peer_max_pdu,
            calling_ae,
            called_ae,
            abstract_syntaxes: Vec::new(),
            max_message_bytes: DEFAULT_MAX_MESSAGE_BYTES,
            next_message_id: 1,
        }
    }

    pub fn next_message_id(&mut self) -> u16 {
        let id = self.next_message_id;
        self.next_message_id = self.next_message_id.wrapping_add(1).max(1);
        id
    }

    /// Akzeptierter Context (ID, Transfer Syntax) für eine Abstract Syntax.
    pub fn context_for(&self, abstract_syntax: &str) -> Option<(u8, &str)> {
        let (id, _) = self
            .abstract_syntaxes
            .iter()
            .find(|(_, a)| a == abstract_syntax)?;
        Some((*id, self.transfer_syntax(*id)?))
    }

    /// Transfer Syntax eines akzeptierten Presentation Contexts.
    pub fn transfer_syntax(&self, context_id: u8) -> Option<&str> {
        self.contexts
            .iter()
            .find(|c| c.id == context_id && c.result == 0)
            .map(|c| c.transfer_syntax.as_str())
    }

    pub fn send(
        &mut self,
        context_id: u8,
        mut command: Dataset,
        data: Option<&[u8]>,
    ) -> Result<(), Error> {
        command.set_u16(
            t::COMMAND_DATA_SET_TYPE,
            if data.is_some() { 0x0000 } else { NO_DATASET },
        );
        let len = command_group_length(&command);
        command.set_u32(t::COMMAND_GROUP_LENGTH, len);
        let cmd = command.encode(false);
        self.send_fragments(context_id, true, &cmd)?;
        if let Some(d) = data {
            self.send_fragments(context_id, false, d)?;
        }
        Ok(())
    }

    fn send_fragments(
        &mut self,
        context_id: u8,
        is_command: bool,
        data: &[u8],
    ) -> Result<(), Error> {
        // PDU-Datenlimit minus PDV-Header (6 Bytes).
        let chunk = (self.peer_max_pdu as usize).saturating_sub(6).max(1024);
        let mut pieces = data.chunks(chunk).peekable();
        if pieces.peek().is_none() {
            let pdv = Pdv {
                context_id,
                is_command,
                is_last: true,
                data: vec![],
            };
            return write_pdu(&mut self.stream, &Pdu::Data(vec![pdv]));
        }
        while let Some(piece) = pieces.next() {
            let pdv = Pdv {
                context_id,
                is_command,
                is_last: pieces.peek().is_none(),
                data: piece.to_vec(),
            };
            write_pdu(&mut self.stream, &Pdu::Data(vec![pdv]))?;
        }
        Ok(())
    }

    /// Liest die nächste vollständige DIMSE-Nachricht.
    /// `Ok(None)` bedeutet: Gegenseite hat Release angefordert (und wir haben
    /// bereits mit Release-RP geantwortet) oder sauber getrennt.
    pub fn receive(&mut self) -> Result<Option<Message>, Error> {
        let mut command_bytes: Vec<u8> = Vec::new();
        let mut data_bytes: Vec<u8> = Vec::new();
        let mut command: Option<Dataset> = None;
        loop {
            let pdu = match read_pdu(&mut self.stream) {
                Ok(p) => p,
                Err(Error::Io(e))
                    if e.kind() == std::io::ErrorKind::UnexpectedEof
                        && command_bytes.is_empty()
                        && command.is_none() =>
                {
                    return Ok(None);
                }
                Err(e) => return Err(e),
            };
            match pdu {
                Pdu::Data(pdvs) => {
                    for pdv in pdvs {
                        let ctx = pdv.context_id;
                        if pdv.is_command {
                            if command_bytes.len() + pdv.data.len() > self.max_message_bytes {
                                return Err(Error::Protocol(
                                    "Command überschreitet das Größenlimit".into(),
                                ));
                            }
                            command_bytes.extend_from_slice(&pdv.data);
                            if pdv.is_last {
                                let c = Dataset::decode(&command_bytes, false)?;
                                let has_data =
                                    c.get_u16(t::COMMAND_DATA_SET_TYPE).unwrap_or(NO_DATASET)
                                        != NO_DATASET;
                                command = Some(c);
                                if !has_data {
                                    return Ok(Some(Message {
                                        context_id: ctx,
                                        command: command.take().unwrap(),
                                        data: None,
                                    }));
                                }
                            }
                        } else {
                            if data_bytes.len() + pdv.data.len() > self.max_message_bytes {
                                return Err(Error::Protocol(format!(
                                    "Datensatz überschreitet das Größenlimit von {} Bytes",
                                    self.max_message_bytes
                                )));
                            }
                            data_bytes.extend_from_slice(&pdv.data);
                            if pdv.is_last {
                                let c = command
                                    .take()
                                    .ok_or_else(|| Error::Protocol("Daten ohne Command".into()))?;
                                return Ok(Some(Message {
                                    context_id: pdv.context_id,
                                    command: c,
                                    data: Some(data_bytes),
                                }));
                            }
                        }
                    }
                }
                Pdu::ReleaseRq => {
                    write_pdu(&mut self.stream, &Pdu::ReleaseRp)?;
                    return Ok(None);
                }
                Pdu::Abort { .. } => return Ok(None),
                other => return Err(Error::Protocol(format!("unerwartetes PDU: {other:?}"))),
            }
        }
    }
}
