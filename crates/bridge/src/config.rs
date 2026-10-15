//! Konfiguration: einfache `schlüssel = wert`-Datei. Unbekannte Schlüssel sind ein
//! Fehler (Tippfehler sollen nicht still ignoriert werden).

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
    pub export_dir: PathBuf,
    /// Pfadpräfix, unter dem das PVS `export_dir` sieht (z. B. `\\\\SRV\\us`).
    pub pvs_export_prefix: String,
    pub settle_seconds: u64,
    pub all_frames: bool,
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
            export_dir: "spool/export".into(),
            pvs_export_prefix: String::new(),
            settle_seconds: 10,
            all_frames: false,
        }
    }
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
            let (k, v) = (k.trim(), v.trim());
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
                "archive_dir" => c.archive_dir = v.into(),
                "export_dir" => c.export_dir = v.into(),
                "pvs_export_prefix" => c.pvs_export_prefix = v.into(),
                "settle_seconds" => c.settle_seconds = parse_num(v).map_err(err)?,
                "all_frames" => c.all_frames = parse_bool(v).map_err(err)?,
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
