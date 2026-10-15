use opengdt_bridge::config::Config;
use opengdt_dicom::scu::{connect, ScuConfig};
use opengdt_dicom::uid;
use std::time::Duration;

const USAGE: &str = "\
opengdt-bridge – DICOM ⇄ GDT-Bridge für Ultraschallgeräte

Verwendung:
  opengdt-bridge run [KONFIG]            Bridge starten (Standard: opengdt.conf)
  opengdt-bridge check-config [KONFIG]   Konfiguration prüfen und ausgeben
  opengdt-bridge echo HOST:PORT [CALLED_AE [CALLING_AE]]
                                         DICOM-Verbindungstest (C-ECHO)
  opengdt-bridge --help
";

fn load_config(path: Option<&String>) -> Result<Config, String> {
    let path = path.map(String::as_str).unwrap_or("opengdt.conf");
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    Config::parse(&text).map_err(|e| format!("{path}: {e}"))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.first().map(String::as_str) {
        Some("run") => match load_config(args.get(1)).and_then(opengdt_bridge::start) {
            Ok(running) => {
                running.wait();
                0
            }
            Err(e) => {
                eprintln!("Fehler: {e}");
                1
            }
        },
        Some("check-config") => match load_config(args.get(1)) {
            Ok(c) => {
                println!("{c:#?}");
                0
            }
            Err(e) => {
                eprintln!("Fehler: {e}");
                1
            }
        },
        Some("echo") => match args.get(1) {
            Some(addr) => {
                let cfg = ScuConfig {
                    called_ae: args.get(2).cloned().unwrap_or_else(|| "OPENGDT".into()),
                    calling_ae: args
                        .get(3)
                        .cloned()
                        .unwrap_or_else(|| "OPENGDT_ECHO".into()),
                    timeout: Duration::from_secs(10),
                };
                let result = connect(
                    addr.as_str(),
                    &cfg,
                    &[(uid::VERIFICATION, &[uid::IMPLICIT_VR_LE])],
                )
                .and_then(|mut a| a.echo().and_then(|s| a.release().map(|_| s)));
                match result {
                    Ok(0) => {
                        println!("C-ECHO erfolgreich");
                        0
                    }
                    Ok(s) => {
                        eprintln!("C-ECHO mit Status {s:#06x}");
                        1
                    }
                    Err(e) => {
                        eprintln!("C-ECHO fehlgeschlagen: {e}");
                        1
                    }
                }
            }
            None => {
                eprint!("{USAGE}");
                2
            }
        },
        _ => {
            eprint!("{USAGE}");
            if matches!(args.first().map(String::as_str), Some("--help" | "-h")) {
                0
            } else {
                2
            }
        }
    };
    std::process::exit(code);
}
