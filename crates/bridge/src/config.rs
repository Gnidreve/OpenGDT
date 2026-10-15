//! Konfiguration: einfache `schlüssel = wert`-Datei. Unbekannte Schlüssel sind ein
//! Fehler (Tippfehler sollen nicht still ignoriert werden).

use opengdt_convert::MultiFrame;
use opengdt_gdt::Charset;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Config {
    // DICOM
    pub ae_title: String,
    pub listen: String,
    pub strict_called_ae: bool,
    // GDT
    pub gdt_import_dir: PathBuf,
    pub gdt_export_dir: PathBuf,
    /// Fester Dateiname für die Antwort (leer = pro Untersuchung eigener Name).
    pub gdt_result_filename: String,
    pub gdt_charset: Charset,
    pub gdt_strict: bool,
    pub gdt_receiver_id: String,
    pub gdt_sender_id: String,
    /// Erwartete Satzart der Anforderung; leer = jede akzeptieren.
    pub gdt_request_type: String,
    pub gdt_result_type: String,
    pub gdt_keep_processed: bool,
    // Worklist
    pub worklist_dir: PathBuf,
    pub modality: String,
    pub station_ae: String,
    pub station_name: String,
    pub procedure_description: String,
    pub utc_offset_minutes: i32,
    pub worklist_ttl_hours: u64,
    pub match_dates: bool,
    // Bilder
    /// Bevorzugte Transfer Syntaxes für eingehende Bilder (UIDs).
    pub storage_transfer_syntaxes: Vec<String>,
    pub archive_dir: PathBuf,
    // Orthanc (optional): leere URL = aus
    pub orthanc_url: String,
    pub orthanc_user: String,
    pub orthanc_password: String,
    pub orthanc_poll_seconds: u64,
    /// Beim ersten Start auch alle bereits vorhandenen Untersuchungen übernehmen.
    pub orthanc_import_history: bool,
    pub export_dir: PathBuf,
    /// Pfadpräfix, unter dem das PVS `export_dir` sieht (z. B. `\\\\SRV\\us`).
    pub pvs_export_prefix: String,
    pub settle_seconds: u64,
    /// Mehrbild-Instanzen: Video (AVI), nur erstes Bild oder alle Bilder einzeln.
    pub multiframe: MultiFrame,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            ae_title: "OPENGDT".into(),
            listen: "0.0.0.0:11112".into(),
            strict_called_ae: false,
            gdt_import_dir: "spool/gdt-in".into(),
            gdt_export_dir: "spool/gdt-out".into(),
            gdt_result_filename: String::new(),
            gdt_charset: Charset::Latin1,
            gdt_strict: false,
            gdt_receiver_id: "PVS".into(),
            gdt_sender_id: "OPENGDT".into(),
            gdt_request_type: String::new(),
            gdt_result_type: "6310".into(),
            gdt_keep_processed: false,
            worklist_dir: "spool/worklist".into(),
            modality: "US".into(),
            station_ae: String::new(),
            station_name: String::new(),
            procedure_description: "Sonographie".into(),
            utc_offset_minutes: 0,
            worklist_ttl_hours: 24,
            match_dates: false,
            storage_transfer_syntaxes: Vec::new(),
            archive_dir: "spool/archive".into(),
            orthanc_url: String::new(),
            orthanc_user: String::new(),
            orthanc_password: String::new(),
            orthanc_poll_seconds: 2,
            orthanc_import_history: false,
            export_dir: "spool/export".into(),
            pvs_export_prefix: String::new(),
            settle_seconds: 10,
            multiframe: MultiFrame::Video,
        }
    }
}

/// Entfernt einen Zeilenkommentar: ab dem ersten `#` (bzw. `;`), das **nach Leerraum** steht.
/// Ein `#` mitten im Wert (z. B. in einem Pfad oder Passwort ohne Leerzeichen davor) bleibt erhalten.
fn strip_comment(v: &str) -> &str {
    let b = v.as_bytes();
    for i in 1..b.len() {
        if (b[i] == b'#' || b[i] == b';') && (b[i - 1] == b' ' || b[i - 1] == b'\t') {
            return &v[..i];
        }
    }
    // Wert besteht nur aus einem Kommentar (z. B. `schlüssel = # leer`)
    if v.starts_with('#') || v.starts_with(';') {
        return "";
    }
    v
}

fn parse_bool(v: &str) -> Result<bool, String> {
    match v.to_ascii_lowercase().as_str() {
        "true" | "yes" | "ja" | "1" | "on" => Ok(true),
        "false" | "no" | "nein" | "0" | "off" => Ok(false),
        _ => Err(format!("'{v}' ist kein Wahrheitswert (true/false)")),
    }
}

fn parse_num<T: std::str::FromStr>(v: &str) -> Result<T, String> {
    v.parse()
        .map_err(|_| format!("'{v}' ist keine gültige Zahl"))
}

fn parse_transfer_syntax(v: &str) -> Result<String, String> {
    use opengdt_dicom::uid;
    match v.to_ascii_lowercase().as_str() {
        "implicit-le" => Ok(uid::IMPLICIT_VR_LE.into()),
        "explicit-le" => Ok(uid::EXPLICIT_VR_LE.into()),
        "jpeg-baseline" => Ok(uid::JPEG_BASELINE.into()),
        _ if v
            .split('.')
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
            && v.contains('.') =>
        {
            Ok(v.to_string())
        }
        _ => Err(format!(
            "unbekannte Transfer Syntax '{v}' (implicit-le, explicit-le, jpeg-baseline oder UID)"
        )),
    }
}

fn parse_multiframe(v: &str) -> Result<MultiFrame, String> {
    match v.to_ascii_lowercase().as_str() {
        "video" | "avi" => Ok(MultiFrame::Video),
        "first" | "erstes" => Ok(MultiFrame::FirstFrame),
        "all" | "alle" => Ok(MultiFrame::AllFrames),
        _ => Err(format!("unbekannter Wert '{v}' (video, first, all)")),
    }
}

fn parse_charset(v: &str) -> Result<Charset, String> {
    match v.to_ascii_lowercase().as_str() {
        "7bit" | "ascii" | "1" => Ok(Charset::SevenBit),
        "cp437" | "ibm" | "2" => Ok(Charset::Cp437),
        "latin1" | "iso8859-1" | "ansi" | "3" => Ok(Charset::Latin1),
        "utf8" | "utf-8" => Ok(Charset::Utf8),
        _ => Err(format!(
            "unbekannter Zeichensatz '{v}' (7bit, cp437, latin1, utf8)"
        )),
    }
}

impl Config {
    pub fn parse(text: &str) -> Result<Config, String> {
        let mut c = Config::default();
        for (i, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            let err = |m: String| format!("Zeile {}: {m}", i + 1);
            let (k, v) = line
                .split_once('=')
                .ok_or_else(|| err("erwartet 'schlüssel = wert'".into()))?;
            let (k, v) = (k.trim(), strip_comment(v).trim());
            match k {
                "ae_title" => c.ae_title = v.into(),
                "listen" => c.listen = v.into(),
                "strict_called_ae" => c.strict_called_ae = parse_bool(v).map_err(err)?,
                "gdt_import_dir" => c.gdt_import_dir = v.into(),
                "gdt_export_dir" => c.gdt_export_dir = v.into(),
                "gdt_result_filename" => c.gdt_result_filename = v.into(),
                "gdt_charset" => c.gdt_charset = parse_charset(v).map_err(err)?,
                "gdt_strict" => c.gdt_strict = parse_bool(v).map_err(err)?,
                "gdt_receiver_id" => c.gdt_receiver_id = v.into(),
                "gdt_sender_id" => c.gdt_sender_id = v.into(),
                "gdt_request_type" => c.gdt_request_type = v.into(),
                "gdt_result_type" => c.gdt_result_type = v.into(),
                "gdt_keep_processed" => c.gdt_keep_processed = parse_bool(v).map_err(err)?,
                "worklist_dir" => c.worklist_dir = v.into(),
                "modality" => c.modality = v.into(),
                "station_ae" => c.station_ae = v.into(),
                "station_name" => c.station_name = v.into(),
                "procedure_description" => c.procedure_description = v.into(),
                "utc_offset_minutes" => c.utc_offset_minutes = parse_num(v).map_err(err)?,
                "worklist_ttl_hours" => c.worklist_ttl_hours = parse_num(v).map_err(err)?,
                "match_dates" => c.match_dates = parse_bool(v).map_err(err)?,
                "storage_transfer_syntaxes" => {
                    c.storage_transfer_syntaxes = v
                        .split(',')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(|s| parse_transfer_syntax(s).map_err(err))
                        .collect::<Result<_, _>>()?
                }
                "orthanc_url" => c.orthanc_url = v.into(),
                "orthanc_user" => c.orthanc_user = v.into(),
                "orthanc_password" => c.orthanc_password = v.into(),
                "orthanc_poll_seconds" => c.orthanc_poll_seconds = parse_num(v).map_err(err)?,
                "orthanc_import_history" => {
                    c.orthanc_import_history = parse_bool(v).map_err(err)?
                }
                "archive_dir" => c.archive_dir = v.into(),
                "export_dir" => c.export_dir = v.into(),
                "pvs_export_prefix" => c.pvs_export_prefix = v.into(),
                "settle_seconds" => c.settle_seconds = parse_num(v).map_err(err)?,
                "multiframe" => c.multiframe = parse_multiframe(v).map_err(err)?,
                other => return Err(err(format!("unbekannter Schlüssel '{other}'"))),
            }
        }
        c.validate()?;
        Ok(c)
    }

    fn validate(&self) -> Result<(), String> {
        if self.ae_title.is_empty() || self.ae_title.len() > 16 || !self.ae_title.is_ascii() {
            return Err("ae_title muss 1–16 ASCII-Zeichen lang sein".into());
        }
        if self.station_ae.len() > 16 {
            return Err("station_ae darf höchstens 16 Zeichen lang sein".into());
        }
        if !self.orthanc_url.is_empty() {
            crate::http::parse_base(&self.orthanc_url).map_err(|e| format!("orthanc_url: {e}"))?;
            if self.orthanc_poll_seconds == 0 {
                return Err("orthanc_poll_seconds muss mindestens 1 sein".into());
            }
        }
        if self.gdt_import_dir == self.gdt_export_dir {
            return Err(
                "gdt_import_dir und gdt_export_dir dürfen nicht identisch sein (Endlosschleife)"
                    .into(),
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trailing_comments_are_stripped() {
        let c = Config::parse("ae_title = ABC   # Kommentar\nstation_ae =   # leer\nlisten = 0.0.0.0:1\t; noch einer\ngdt_import_dir = /data/a#b").unwrap();
        assert_eq!(c.ae_title, "ABC");
        assert_eq!(c.station_ae, "");
        assert_eq!(c.listen, "0.0.0.0:1");
        assert_eq!(
            c.gdt_import_dir,
            std::path::PathBuf::from("/data/a#b"),
            "# ohne Leerraum davor gehört zum Wert"
        );
    }

    /// Alle ausgelieferten Konfigurationsdateien müssen sich laden lassen (die README sagt „kopieren und anpassen“).
    #[test]
    fn shipped_config_files_parse() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
        for f in ["crates/bridge/opengdt.conf.example", "docker/bridge.conf"] {
            let text = std::fs::read_to_string(format!("{root}/{f}")).unwrap();
            Config::parse(&text).unwrap_or_else(|e| panic!("{f}: {e}"));
        }
    }

    #[test]
    fn defaults_are_valid() {
        assert!(Config::default().validate().is_ok());
    }

    #[test]
    fn parses_values_and_comments() {
        let c = Config::parse("# Kommentar\nae_title = TESTAE\nstrict_called_ae = ja\ngdt_charset = cp437\nutc_offset_minutes = -60\n").unwrap();
        assert_eq!(c.ae_title, "TESTAE");
        assert!(c.strict_called_ae);
        assert_eq!(c.gdt_charset, Charset::Cp437);
        assert_eq!(c.utc_offset_minutes, -60);
    }

    #[test]
    fn orthanc_settings() {
        let c = Config::parse("orthanc_url = http://orthanc:8042\northanc_user = u\northanc_password = p\northanc_poll_seconds = 5").unwrap();
        assert_eq!(
            (c.orthanc_url.as_str(), c.orthanc_poll_seconds),
            ("http://orthanc:8042", 5)
        );
        assert!(
            Config::default().orthanc_url.is_empty(),
            "standardmäßig aus"
        );
        assert!(Config::parse("orthanc_url = https://x").is_err());
        assert!(Config::parse("orthanc_url = http://x\northanc_poll_seconds = 0").is_err());
    }

    #[test]
    fn parses_multiframe() {
        assert_eq!(Config::default().multiframe, MultiFrame::Video);
        assert_eq!(
            Config::parse("multiframe = first").unwrap().multiframe,
            MultiFrame::FirstFrame
        );
        assert_eq!(
            Config::parse("multiframe = ALL").unwrap().multiframe,
            MultiFrame::AllFrames
        );
        assert!(Config::parse("multiframe = gif").is_err());
        assert!(
            Config::parse("all_frames = true").is_err(),
            "alter Schlüssel ist entfernt"
        );
    }

    #[test]
    fn parses_transfer_syntax_list() {
        let c = Config::parse(
            "storage_transfer_syntaxes = jpeg-baseline, explicit-le, 1.2.840.10008.1.2.5",
        )
        .unwrap();
        assert_eq!(c.storage_transfer_syntaxes.len(), 3);
        assert_eq!(c.storage_transfer_syntaxes[0], "1.2.840.10008.1.2.4.50");
        assert!(Config::parse("storage_transfer_syntaxes = quatsch").is_err());
        assert!(Config::parse("storage_transfer_syntaxes =")
            .unwrap()
            .storage_transfer_syntaxes
            .is_empty());
    }

    #[test]
    fn rejects_typos_and_bad_values() {
        assert!(Config::parse("ae_titel = X")
            .unwrap_err()
            .contains("unbekannter Schlüssel"));
        assert!(Config::parse("settle_seconds = abc").is_err());
        assert!(Config::parse("ae_title = DASISTVIELZULANGEREINAETITLE").is_err());
        assert!(Config::parse("gdt_import_dir = x\ngdt_export_dir = x").is_err());
        assert!(Config::parse("nur text")
            .unwrap_err()
            .starts_with("Zeile 1"));
    }
}

#[cfg(test)]
mod example_matches_defaults {
    use super::*;

    /// Die Beispieldatei behauptet „hier stehen die Standardwerte“ – das muss stimmen.
    #[test]
    fn example_file_shows_the_real_defaults() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
        let text =
            std::fs::read_to_string(format!("{root}/crates/bridge/opengdt.conf.example")).unwrap();
        let parsed = Config::parse(&text).unwrap();
        assert_eq!(format!("{parsed:#?}"), format!("{:#?}", Config::default()));
    }
}
