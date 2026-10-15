//! DICOM Upper Layer PDUs (PS3.8).

use crate::uid;
use crate::Error;
use std::io::{Read, Write};

/// Obergrenze für PDU-Größen, die wir lesen (Schutz vor Speicherbomben).
const MAX_PDU: u32 = 16 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresentationContextRq {
    pub id: u8,
    pub abstract_syntax: String,
    pub transfer_syntaxes: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresentationContextAc {
    pub id: u8,
    /// 0 = Annahme, 1 = Benutzer lehnt ab, 2 = Provider ohne Grund,
    /// 3 = Abstract Syntax nicht unterstützt, 4 = Transfer Syntax nicht unterstützt
    pub result: u8,
    pub transfer_syntax: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pdv {
    pub context_id: u8,
    pub is_command: bool,
    pub is_last: bool,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pdu {
    AssociateRq {
        called_ae: String,
        calling_ae: String,
        contexts: Vec<PresentationContextRq>,
        max_pdu: u32,
    },
    AssociateAc {
        called_ae: String,
        calling_ae: String,
        contexts: Vec<PresentationContextAc>,
        max_pdu: u32,
    },
    AssociateRj {
        result: u8,
        source: u8,
        reason: u8,
    },
    Data(Vec<Pdv>),
    ReleaseRq,
    ReleaseRp,
    Abort {
        source: u8,
        reason: u8,
    },
}

// ---------------------------------------------------------------- Schreiben

pub fn write_pdu<W: Write>(w: &mut W, pdu: &Pdu) -> Result<(), Error> {
    let (ty, body) = match pdu {
        Pdu::AssociateRq {
            called_ae,
            calling_ae,
            contexts,
            max_pdu,
        } => {
            let mut b = assoc_header(called_ae, calling_ae);
            put_item(&mut b, 0x10, uid::APPLICATION_CONTEXT.as_bytes());
            for c in contexts {
                let mut cb = vec![c.id, 0, 0, 0];
                put_item(&mut cb, 0x30, c.abstract_syntax.as_bytes());
                for ts in &c.transfer_syntaxes {
                    put_item(&mut cb, 0x40, ts.as_bytes());
                }
                put_item(&mut b, 0x20, &cb);
            }
            put_item(&mut b, 0x50, &user_info(*max_pdu));
            (0x01, b)
        }
        Pdu::AssociateAc {
            called_ae,
            calling_ae,
            contexts,
            max_pdu,
        } => {
            let mut b = assoc_header(called_ae, calling_ae);
            put_item(&mut b, 0x10, uid::APPLICATION_CONTEXT.as_bytes());
            for c in contexts {
                let mut cb = vec![c.id, 0, c.result, 0];
                put_item(&mut cb, 0x40, c.transfer_syntax.as_bytes());
                put_item(&mut b, 0x21, &cb);
            }
            put_item(&mut b, 0x50, &user_info(*max_pdu));
            (0x02, b)
        }
        Pdu::AssociateRj {
            result,
            source,
            reason,
        } => (0x03, vec![0, *result, *source, *reason]),
        Pdu::Data(pdvs) => {
            let mut b = Vec::new();
            for p in pdvs {
                b.extend_from_slice(&((p.data.len() as u32 + 2).to_be_bytes()));
                b.push(p.context_id);
                b.push((p.is_command as u8) | ((p.is_last as u8) << 1));
                b.extend_from_slice(&p.data);
            }
            (0x04, b)
        }
        Pdu::ReleaseRq => (0x05, vec![0; 4]),
        Pdu::ReleaseRp => (0x06, vec![0; 4]),
        Pdu::Abort { source, reason } => (0x07, vec![0, 0, *source, *reason]),
    };
    let mut out = Vec::with_capacity(6 + body.len());
    out.push(ty);
    out.push(0);
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(&body);
    w.write_all(&out)?;
    w.flush()?;
    Ok(())
}

fn ae16(ae: &str) -> [u8; 16] {
    let mut a = [b' '; 16];
    for (i, b) in ae.bytes().take(16).enumerate() {
        a[i] = b;
    }
    a
}

fn assoc_header(called: &str, calling: &str) -> Vec<u8> {
    let mut b = vec![0, 1, 0, 0];
    b.extend_from_slice(&ae16(called));
    b.extend_from_slice(&ae16(calling));
    b.extend_from_slice(&[0; 32]);
    b
}

fn put_item(out: &mut Vec<u8>, ty: u8, body: &[u8]) {
    out.push(ty);
    out.push(0);
    out.extend_from_slice(&(body.len() as u16).to_be_bytes());
    out.extend_from_slice(body);
}

fn user_info(max_pdu: u32) -> Vec<u8> {
    let mut u = Vec::new();
    put_item(&mut u, 0x51, &max_pdu.to_be_bytes());
    put_item(&mut u, 0x52, uid::IMPLEMENTATION_CLASS.as_bytes());
    put_item(&mut u, 0x55, uid::IMPLEMENTATION_VERSION.as_bytes());
    u
}

// ------------------------------------------------------------------- Lesen

pub fn read_pdu<R: Read>(r: &mut R) -> Result<Pdu, Error> {
    let mut head = [0u8; 6];
    r.read_exact(&mut head)?;
    let len = u32::from_be_bytes([head[2], head[3], head[4], head[5]]);
    if len > MAX_PDU {
        return Err(Error::Protocol(format!(
            "PDU mit {len} Bytes überschreitet das Limit"
        )));
    }
    let mut body = vec![0u8; len as usize];
    r.read_exact(&mut body)?;
    parse_pdu(head[0], &body)
}

fn parse_pdu(ty: u8, b: &[u8]) -> Result<Pdu, Error> {
    match ty {
        0x01 | 0x02 => parse_associate(ty, b),
        0x03 => {
            need(b, 4)?;
            Ok(Pdu::AssociateRj {
                result: b[1],
                source: b[2],
                reason: b[3],
            })
        }
        0x04 => {
            let mut pdvs = Vec::new();
            let mut p = 0;
            while p < b.len() {
                need(&b[p..], 6)?;
                let len = u32::from_be_bytes([b[p], b[p + 1], b[p + 2], b[p + 3]]) as usize;
                if len < 2 || p + 4 + len > b.len() {
                    return Err(Error::Protocol("PDV-Länge ungültig".into()));
                }
                let flags = b[p + 5];
                pdvs.push(Pdv {
                    context_id: b[p + 4],
                    is_command: flags & 1 != 0,
                    is_last: flags & 2 != 0,
                    data: b[p + 6..p + 4 + len].to_vec(),
                });
                p += 4 + len;
            }
            Ok(Pdu::Data(pdvs))
        }
        0x05 => Ok(Pdu::ReleaseRq),
        0x06 => Ok(Pdu::ReleaseRp),
        0x07 => {
            need(b, 4)?;
            Ok(Pdu::Abort {
                source: b[2],
                reason: b[3],
            })
        }
        _ => Err(Error::Protocol(format!("unbekannter PDU-Typ {ty:#04x}"))),
    }
}

fn need(b: &[u8], n: usize) -> Result<(), Error> {
    if b.len() < n {
        Err(Error::Protocol("PDU zu kurz".into()))
    } else {
        Ok(())
    }
}

struct Item<'a> {
    ty: u8,
    body: &'a [u8],
}

fn items(mut b: &[u8]) -> Result<Vec<Item<'_>>, Error> {
    let mut v = Vec::new();
    while !b.is_empty() {
        need(b, 4)?;
        let len = u16::from_be_bytes([b[2], b[3]]) as usize;
        if 4 + len > b.len() {
            return Err(Error::Protocol("Item-Länge ungültig".into()));
        }
        v.push(Item {
            ty: b[0],
            body: &b[4..4 + len],
        });
        b = &b[4 + len..];
    }
    Ok(v)
}

fn uid_str(b: &[u8]) -> String {
    String::from_utf8_lossy(b)
        .trim_end_matches(['\0', ' '])
        .to_string()
}

fn parse_associate(ty: u8, b: &[u8]) -> Result<Pdu, Error> {
    need(b, 68)?;
    let called_ae = String::from_utf8_lossy(&b[4..20]).trim().to_string();
    let calling_ae = String::from_utf8_lossy(&b[20..36]).trim().to_string();
    let mut rq = Vec::new();
    let mut ac = Vec::new();
    let mut max_pdu = 16384;
    for it in items(&b[68..])? {
        match it.ty {
            0x20 => {
                need(it.body, 4)?;
                let mut c = PresentationContextRq {
                    id: it.body[0],
                    abstract_syntax: String::new(),
                    transfer_syntaxes: vec![],
                };
                for s in items(&it.body[4..])? {
                    match s.ty {
                        0x30 => c.abstract_syntax = uid_str(s.body),
                        0x40 => c.transfer_syntaxes.push(uid_str(s.body)),
                        _ => {}
                    }
                }
                rq.push(c);
            }
            0x21 => {
                need(it.body, 4)?;
                let mut ts = String::new();
                for s in items(&it.body[4..])? {
                    if s.ty == 0x40 {
                        ts = uid_str(s.body);
                    }
                }
                ac.push(PresentationContextAc {
                    id: it.body[0],
                    result: it.body[2],
                    transfer_syntax: ts,
                });
            }
            0x50 => {
                for s in items(it.body)? {
                    if s.ty == 0x51 && s.body.len() == 4 {
                        let m = u32::from_be_bytes([s.body[0], s.body[1], s.body[2], s.body[3]]);
                        // 0 = unbegrenzt; wir deckeln sinnvoll.
                        max_pdu = if m == 0 { 1 << 20 } else { m.min(1 << 20) };
                    }
                }
            }
            _ => {}
        }
    }
    Ok(if ty == 1 {
        Pdu::AssociateRq {
            called_ae,
            calling_ae,
            contexts: rq,
            max_pdu,
        }
    } else {
        Pdu::AssociateAc {
            called_ae,
            calling_ae,
            contexts: ac,
            max_pdu,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rt(p: Pdu) -> Pdu {
        let mut buf = Vec::new();
        write_pdu(&mut buf, &p).unwrap();
        read_pdu(&mut buf.as_slice()).unwrap()
    }

    #[test]
    fn associate_rq_roundtrip() {
        let p = Pdu::AssociateRq {
            called_ae: "OPENGDT".into(),
            calling_ae: "MINDRAY".into(),
            contexts: vec![PresentationContextRq {
                id: 1,
                abstract_syntax: uid::VERIFICATION.into(),
                transfer_syntaxes: vec![uid::IMPLICIT_VR_LE.into(), uid::EXPLICIT_VR_LE.into()],
            }],
            max_pdu: 16384,
        };
        assert_eq!(rt(p.clone()), p);
    }

    #[test]
    fn associate_ac_and_data_roundtrip() {
        let p = Pdu::AssociateAc {
            called_ae: "A".into(),
            calling_ae: "B".into(),
            contexts: vec![PresentationContextAc {
                id: 3,
                result: 0,
                transfer_syntax: uid::IMPLICIT_VR_LE.into(),
            }],
            max_pdu: 16384,
        };
        assert_eq!(rt(p.clone()), p);
        let d = Pdu::Data(vec![Pdv {
            context_id: 1,
            is_command: true,
            is_last: true,
            data: vec![1, 2, 3],
        }]);
        assert_eq!(rt(d.clone()), d);
        assert_eq!(rt(Pdu::ReleaseRq), Pdu::ReleaseRq);
        assert_eq!(
            rt(Pdu::Abort {
                source: 2,
                reason: 0
            }),
            Pdu::Abort {
                source: 2,
                reason: 0
            }
        );
    }

    #[test]
    fn oversized_pdu_is_rejected() {
        let mut buf = vec![4, 0];
        buf.extend_from_slice(&(MAX_PDU + 1).to_be_bytes());
        assert!(read_pdu(&mut buf.as_slice()).is_err());
    }
}
