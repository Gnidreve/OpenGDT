//! Minimaler Service Class User – für Selbsttests und Diagnose-Kommandos
//! (`bridge echo`), nicht als vollwertiger Client gedacht.

use crate::assoc::*;
use crate::dataset::Dataset;
use crate::file::is_explicit;
use crate::pdu::{read_pdu, write_pdu, Pdu, PresentationContextRq};
use crate::tag::t;
use crate::{uid, Error};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

pub struct ScuConfig {
    pub calling_ae: String,
    pub called_ae: String,
    pub timeout: Duration,
}

/// Baut eine Association auf. `syntaxes`: (Abstract Syntax, vorgeschlagene Transfer Syntaxes).
pub fn connect<A: ToSocketAddrs>(
    addr: A,
    cfg: &ScuConfig,
    syntaxes: &[(&str, &[&str])],
) -> Result<Association, Error> {
    let mut stream = TcpStream::connect(addr)?;
    stream.set_read_timeout(Some(cfg.timeout))?;
    stream.set_write_timeout(Some(cfg.timeout))?;
    let contexts: Vec<PresentationContextRq> = syntaxes
        .iter()
        .enumerate()
        .map(|(i, (a, ts))| PresentationContextRq {
            id: (2 * i + 1) as u8,
            abstract_syntax: a.to_string(),
            transfer_syntaxes: ts.iter().map(|s| s.to_string()).collect(),
        })
        .collect();
    write_pdu(
        &mut stream,
        &Pdu::AssociateRq {
            called_ae: cfg.called_ae.clone(),
            calling_ae: cfg.calling_ae.clone(),
            contexts: contexts.clone(),
            max_pdu: 16384,
        },
    )?;
    match read_pdu(&mut stream)? {
        Pdu::AssociateAc {
            contexts: ac,
            max_pdu,
            ..
        } => {
            let mut a = Association::new(
                stream,
                ac,
                max_pdu,
                cfg.calling_ae.clone(),
                cfg.called_ae.clone(),
            );
            a.abstract_syntaxes = contexts
                .iter()
                .map(|c| (c.id, c.abstract_syntax.clone()))
                .collect();
            Ok(a)
        }
        Pdu::AssociateRj {
            result,
            source,
            reason,
        } => Err(Error::Rejected(format!(
            "A-ASSOCIATE-RJ result={result} source={source} reason={reason}"
        ))),
        other => Err(Error::Protocol(format!("unerwartet: {other:?}"))),
    }
}

impl Association<TcpStream> {
    fn request(
        &mut self,
        abstract_syntax: &str,
        cmd_field: u16,
        sop_instance: Option<&str>,
    ) -> Result<(u8, Dataset), Error> {
        let (ctx, _) = self.context_for(abstract_syntax).ok_or_else(|| {
            Error::Rejected(format!(
                "Abstract Syntax {abstract_syntax} nicht akzeptiert"
            ))
        })?;
        let mut c = Dataset::new();
        c.set_str(t::AFFECTED_SOP_CLASS_UID, abstract_syntax);
        c.set_u16(t::COMMAND_FIELD, cmd_field);
        let id = self.next_message_id();
        c.set_u16(t::MESSAGE_ID, id);
        if cmd_field != CMD_C_ECHO_RQ {
            c.set_u16(t::PRIORITY, 0);
        }
        if let Some(i) = sop_instance {
            c.set_str(t::AFFECTED_SOP_INSTANCE_UID, i);
        }
        Ok((ctx, c))
    }

    pub fn echo(&mut self) -> Result<u16, Error> {
        let (ctx, cmd) = self.request(uid::VERIFICATION, CMD_C_ECHO_RQ, None)?;
        self.send(ctx, cmd, None)?;
        let rsp = self
            .receive()?
            .ok_or_else(|| Error::Protocol("keine Antwort".into()))?;
        Ok(rsp.command.get_u16(t::STATUS).unwrap_or(0xFFFF))
    }

    pub fn store(
        &mut self,
        sop_class: &str,
        sop_instance: &str,
        data: &[u8],
    ) -> Result<u16, Error> {
        let (ctx, cmd) = self.request(sop_class, CMD_C_STORE_RQ, Some(sop_instance))?;
        self.send(ctx, cmd, Some(data))?;
        let rsp = self
            .receive()?
            .ok_or_else(|| Error::Protocol("keine Antwort".into()))?;
        Ok(rsp.command.get_u16(t::STATUS).unwrap_or(0xFFFF))
    }

    /// C-FIND (Worklist). Gibt (Status, Treffer) zurück.
    pub fn find(&mut self, identifier: &Dataset) -> Result<(u16, Vec<Dataset>), Error> {
        let (ctx, cmd) = self.request(uid::MWL_FIND, CMD_C_FIND_RQ, None)?;
        let ts = self
            .transfer_syntax(ctx)
            .unwrap_or(uid::IMPLICIT_VR_LE)
            .to_string();
        let explicit = is_explicit(&ts).unwrap_or(false);
        self.send(ctx, cmd, Some(&identifier.encode(explicit)))?;
        let mut out = Vec::new();
        loop {
            let rsp = self
                .receive()?
                .ok_or_else(|| Error::Protocol("Verbindung vorzeitig beendet".into()))?;
            let status = rsp.command.get_u16(t::STATUS).unwrap_or(0xFFFF);
            if status == STATUS_PENDING || status == 0xFF01 {
                if let Some(d) = rsp.data {
                    out.push(Dataset::decode(&d, explicit)?);
                }
            } else {
                return Ok((status, out));
            }
        }
    }

    pub fn release(mut self) -> Result<(), Error> {
        write_pdu(&mut self.stream, &Pdu::ReleaseRq)?;
        match read_pdu(&mut self.stream)? {
            Pdu::ReleaseRp => Ok(()),
            other => Err(Error::Protocol(format!(
                "unerwartet nach Release: {other:?}"
            ))),
        }
    }
}
