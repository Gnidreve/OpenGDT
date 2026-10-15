//! Minimaler HTTP/1.0-Client (nur GET, nur `http://`) auf `std`-Basis.
//!
//! HTTP/1.0 mit `Connection: close` vermeidet Keep-Alive und (meist) Chunked-Antworten; die Antwort
//! ist der gesamte Rest der Verbindung. Falls ein Server trotzdem `chunked` sendet, wird das dekodiert.

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

/// Obergrenze für eine Antwort (Schutz vor Speicherbomben; Cine-Clips können groß sein).
pub const MAX_BODY: usize = 512 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Base {
    pub host: String,
    pub port: u16,
    /// Pfadpräfix ohne abschließenden Schrägstrich, z. B. `/orthanc`
    pub prefix: String,
}

#[derive(Debug)]
pub struct Response {
    pub status: u16,
    pub body: Vec<u8>,
}

/// `http://host[:port][/prefix]`
pub fn parse_base(url: &str) -> Result<Base, String> {
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| format!("'{url}': nur http:// wird unterstützt"))?;
    let (hostport, prefix) = match rest.find('/') {
        Some(i) => (&rest[..i], rest[i..].trim_end_matches('/')),
        None => (rest, ""),
    };
    let (host, port) = match hostport.rsplit_once(':') {
        Some((h, p)) => (
            h,
            p.parse::<u16>()
                .map_err(|_| format!("'{url}': ungültiger Port"))?,
        ),
        None => (hostport, 80),
    };
    if host.is_empty() {
        return Err(format!("'{url}': Host fehlt"));
    }
    Ok(Base {
        host: host.to_string(),
        port,
        prefix: prefix.to_string(),
    })
}

/// Wert für den `Authorization`-Header (Basic).
pub fn basic_auth(user: &str, password: &str) -> String {
    format!("Basic {}", base64(format!("{user}:{password}").as_bytes()))
}

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16
            | (*c.get(1).unwrap_or(&0) as u32) << 8
            | *c.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if c.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

pub fn get(
    base: &Base,
    path: &str,
    auth: Option<&str>,
    timeout: Duration,
) -> Result<Response, String> {
    let addr = (base.host.as_str(), base.port)
        .to_socket_addrs()
        .map_err(|e| format!("{}: {e}", base.host))?
        .next()
        .ok_or_else(|| format!("{}: keine Adresse", base.host))?;
    let mut s = TcpStream::connect_timeout(&addr, Duration::from_secs(5))
        .map_err(|e| format!("{addr}: {e}"))?;
    s.set_read_timeout(Some(timeout))
        .map_err(|e| e.to_string())?;
    s.set_write_timeout(Some(timeout))
        .map_err(|e| e.to_string())?;
    let mut req = format!(
        "GET {}{path} HTTP/1.0\r\nHost: {}:{}\r\nAccept: */*\r\nConnection: close\r\n",
        base.prefix, base.host, base.port
    );
    if let Some(a) = auth {
        req.push_str(&format!("Authorization: {a}\r\n"));
    }
    req.push_str("\r\n");
    s.write_all(req.as_bytes()).map_err(|e| e.to_string())?;

    let mut raw = Vec::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        match s.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                raw.extend_from_slice(&buf[..n]);
                if raw.len() > MAX_BODY + 64 * 1024 {
                    return Err("Antwort überschreitet das Größenlimit".into());
                }
            }
            Err(e) => return Err(e.to_string()),
        }
    }
    parse_response(&raw)
}

fn parse_response(raw: &[u8]) -> Result<Response, String> {
    let end = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or("Antwort ohne vollständigen Header")?;
    let head = String::from_utf8_lossy(&raw[..end]).to_string();
    let mut lines = head.lines();
    let status_line = lines.next().unwrap_or("");
    let status: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| format!("ungültige Statuszeile '{status_line}'"))?;
    let chunked = lines.any(|l| {
        let l = l.to_ascii_lowercase();
        l.starts_with("transfer-encoding:") && l.contains("chunked")
    });
    let body = &raw[end + 4..];
    let body = if chunked {
        dechunk(body)?
    } else {
        body.to_vec()
    };
    if body.len() > MAX_BODY {
        return Err("Antwort überschreitet das Größenlimit".into());
    }
    Ok(Response { status, body })
}

fn dechunk(mut b: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    loop {
        let eol = b
            .windows(2)
            .position(|w| w == b"\r\n")
            .ok_or("Chunk-Kopf abgeschnitten")?;
        let size_txt = String::from_utf8_lossy(&b[..eol]).to_string();
        let size_txt = size_txt.split(';').next().unwrap_or("").trim().to_string();
        let size = usize::from_str_radix(&size_txt, 16)
            .map_err(|_| format!("ungültige Chunk-Größe '{size_txt}'"))?;
        b = &b[eol + 2..];
        if size == 0 {
            return Ok(out);
        }
        if b.len() < size + 2 {
            return Err("Chunk abgeschnitten".into());
        }
        out.extend_from_slice(&b[..size]);
        b = &b[size + 2..];
        if out.len() > MAX_BODY {
            return Err("Antwort überschreitet das Größenlimit".into());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_base_urls() {
        assert_eq!(
            parse_base("http://orthanc:8042").unwrap(),
            Base {
                host: "orthanc".into(),
                port: 8042,
                prefix: "".into()
            }
        );
        assert_eq!(parse_base("http://h/").unwrap().prefix, "");
        assert_eq!(
            parse_base("http://h:1/orthanc/").unwrap().prefix,
            "/orthanc"
        );
        assert_eq!(parse_base("http://h").unwrap().port, 80);
        for bad in [
            "https://h",
            "h:1",
            "http://",
            "http://:80",
            "http://h:abc",
            "http://h:99999",
        ] {
            assert!(parse_base(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn base64_vectors() {
        // RFC 4648
        for (i, o) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(i.as_bytes()), o);
        }
        assert_eq!(
            basic_auth("Aladdin", "open sesame"),
            "Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ=="
        );
    }

    #[test]
    fn parses_plain_and_chunked_responses() {
        let r = parse_response(b"HTTP/1.0 200 OK\r\nContent-Length: 3\r\n\r\nabc").unwrap();
        assert_eq!((r.status, r.body.as_slice()), (200, &b"abc"[..]));
        let r = parse_response(b"HTTP/1.1 404 Not Found\r\n\r\n").unwrap();
        assert_eq!((r.status, r.body.len()), (404, 0));
        let r = parse_response(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n4;ext=1\r\ndefg\r\n0\r\n\r\n").unwrap();
        assert_eq!(r.body, b"abcdefg");
    }

    #[test]
    fn response_parser_survives_garbage() {
        let seeds: [&[u8]; 3] = [
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n4;ext=1\r\ndefg\r\n0\r\n\r\n",
            b"HTTP/1.0 200 OK\r\nContent-Length: 3\r\n\r\nabc",
            b"HTTP/1.1 404 Not Found\r\n\r\n",
        ];
        let mut x: u64 = 0x1357_9BDF_2468_ACE0;
        let mut next = || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x
        };
        for i in 0..80_000 {
            let mut v = seeds[(next() % 3) as usize].to_vec();
            for _ in 0..1 + next() % 5 {
                if v.is_empty() {
                    break;
                }
                let at = (next() % v.len() as u64) as usize;
                match next() % 4 {
                    0 => v[at] = next() as u8,
                    1 => {
                        v.remove(at);
                    }
                    2 => v.truncate(at),
                    _ => {
                        const FILL: &[u8] = b"\r\n0123456789abcfF;";
                        v.insert(at, FILL[(next() % FILL.len() as u64) as usize])
                    }
                }
            }
            let r = std::panic::catch_unwind(|| {
                let _ = parse_response(&v);
            });
            assert!(
                r.is_ok(),
                "Panic bei Durchlauf {i}: {:?}",
                String::from_utf8_lossy(&v)
            );
        }
    }

    #[test]
    fn rejects_broken_responses() {
        for bad in [
            &b""[..],
            b"HTTP/1.0 200 OK",
            b"quatsch\r\n\r\n",
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nzz\r\n",
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nab",
        ] {
            assert!(
                parse_response(bad).is_err(),
                "{:?}",
                String::from_utf8_lossy(bad)
            );
        }
    }
}
