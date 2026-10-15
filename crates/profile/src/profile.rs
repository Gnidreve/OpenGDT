use crate::json::Json;
use crate::SCHEMA;
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Meta {
    pub profile_id: String,
    pub source_url: String,
    pub title: String,
    pub manufacturer: String,
    pub part_number: String,
    pub document_version: String,
    /// `YYYY-MM-DD`
    pub published: String,
    pub file_name: String,
    pub file_sha256: String,
    /// `YYYY-MM-DDTHH:MM:SSZ`
    pub extracted_at: String,
    pub extracted_by: String,
    /// `unreviewed` oder `reviewed`
    pub review_status: String,
    pub verified_against_device: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransferSyntax {
    pub key: String,
    pub name: String,
    pub uid: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StorageContext {
    pub name: String,
    pub uid: String,
    pub transfer_syntaxes: Vec<TransferSyntax>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SopClass {
    pub name: String,
    pub uid: String,
    pub scu: bool,
    pub scp: bool,
}

/// Ein Worklist-Schlüssel (Matching- und/oder Return-Key), ggf. mit Unter-Schlüsseln.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Key {
    pub tag: (u16, u16),
    pub vr: [u8; 2],
    pub name: String,
    /// z. B. `S`, `*`, `DA-range`
    pub matching: Vec<String>,
    pub is_return: bool,
    pub fixed_value: Option<String>,
    pub items: Vec<Key>,
}

pub struct Profile {
    root: Json,
}

const VRS: &[&str] = &[
    "AE", "AS", "AT", "CS", "DA", "DS", "DT", "FD", "FL", "IS", "LO", "LT", "OB", "OD", "OF", "OW",
    "PN", "SH", "SL", "SQ", "SS", "ST", "TM", "UI", "UL", "UN", "US", "UT",
];

impl Profile {
    /// Parst und **validiert**. Bei Fehlern kommen alle gefundenen Probleme zurück.
    pub fn parse(text: &str) -> Result<Profile, Vec<String>> {
        let root = Json::parse(text).map_err(|e| vec![e])?;
        let p = Profile { root };
        let errs = p.validate();
        if errs.is_empty() {
            Ok(p)
        } else {
            Err(errs)
        }
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Profile, Vec<String>> {
        let path = path.as_ref();
        let text =
            std::fs::read_to_string(path).map_err(|e| vec![format!("{}: {e}", path.display())])?;
        Profile::parse(&text)
    }

    pub fn raw(&self) -> &Json {
        &self.root
    }

    fn s(&self, path: &str) -> String {
        self.root
            .at(path)
            .and_then(Json::as_str)
            .unwrap_or_default()
            .to_string()
    }

    pub fn meta(&self) -> Meta {
        Meta {
            profile_id: self.s("meta.profile_id"),
            source_url: self.s("meta.source.url"),
            title: self.s("meta.source.title"),
            manufacturer: self.s("meta.source.manufacturer"),
            part_number: self.s("meta.source.part_number"),
            document_version: self.s("meta.source.document_version"),
            published: self.s("meta.source.published"),
            file_name: self.s("meta.source.file_name"),
            file_sha256: self.s("meta.source.file_sha256"),
            extracted_at: self.s("meta.capture.extracted_at"),
            extracted_by: self.s("meta.capture.extracted_by"),
            review_status: self.s("meta.capture.review_status"),
            verified_against_device: self
                .root
                .at("meta.capture.verified_against_device")
                .and_then(Json::as_bool)
                .unwrap_or(false),
        }
    }

    fn transfer_syntaxes(j: Option<&Json>) -> Vec<TransferSyntax> {
        j.and_then(Json::as_arr)
            .unwrap_or_default()
            .iter()
            .map(|t| TransferSyntax {
                key: t
                    .get("key")
                    .and_then(Json::as_str)
                    .unwrap_or_default()
                    .into(),
                name: t
                    .get("name")
                    .and_then(Json::as_str)
                    .unwrap_or_default()
                    .into(),
                uid: t
                    .get("uid")
                    .and_then(Json::as_str)
                    .unwrap_or_default()
                    .into(),
            })
            .collect()
    }

    pub fn worklist_transfer_syntaxes(&self) -> Vec<TransferSyntax> {
        Self::transfer_syntaxes(self.root.at("worklist.proposed_transfer_syntaxes"))
    }

    pub fn storage_contexts(&self) -> Vec<StorageContext> {
        self.root
            .at("storage.proposed_contexts")
            .and_then(Json::as_arr)
            .unwrap_or_default()
            .iter()
            .map(|c| StorageContext {
                name: c
                    .get("abstract_syntax")
                    .and_then(Json::as_str)
                    .unwrap_or_default()
                    .into(),
                uid: c
                    .get("uid")
                    .and_then(Json::as_str)
                    .unwrap_or_default()
                    .into(),
                transfer_syntaxes: Self::transfer_syntaxes(c.get("transfer_syntaxes")),
            })
            .collect()
    }

    pub fn sop_classes(&self) -> Vec<SopClass> {
        self.root
            .get("sop_classes")
            .and_then(Json::as_arr)
            .unwrap_or_default()
            .iter()
            .map(|c| SopClass {
                name: c
                    .get("name")
                    .and_then(Json::as_str)
                    .unwrap_or_default()
                    .into(),
                uid: c
                    .get("uid")
                    .and_then(Json::as_str)
                    .unwrap_or_default()
                    .into(),
                scu: c.get("scu").and_then(Json::as_bool).unwrap_or(false),
                scp: c.get("scp").and_then(Json::as_bool).unwrap_or(false),
            })
            .collect()
    }

    /// Erste SOP-Klasse, deren Name den Text enthält (Groß-/Kleinschreibung egal).
    pub fn find_sop_class(&self, name_part: &str) -> Option<SopClass> {
        let n = name_part.to_lowercase();
        self.sop_classes()
            .into_iter()
            .find(|c| c.name.to_lowercase().contains(&n))
    }

    pub fn worklist_keys(&self) -> Vec<Key> {
        Self::keys(self.root.at("worklist.keys"))
    }

    fn keys(j: Option<&Json>) -> Vec<Key> {
        j.and_then(Json::as_arr)
            .unwrap_or_default()
            .iter()
            .filter_map(|k| {
                let tag = parse_tag(k.get("tag")?.as_str()?)?;
                let vr = k.get("vr")?.as_str()?.as_bytes();
                Some(Key {
                    tag,
                    vr: [*vr.first()?, *vr.get(1)?],
                    name: k
                        .get("name")
                        .and_then(Json::as_str)
                        .unwrap_or_default()
                        .into(),
                    matching: k
                        .get("matching")
                        .and_then(Json::as_arr)
                        .unwrap_or_default()
                        .iter()
                        .filter_map(|m| m.as_str().map(String::from))
                        .collect(),
                    is_return: k.get("return").and_then(Json::as_bool).unwrap_or(false),
                    fixed_value: k
                        .get("fixed_value")
                        .and_then(Json::as_str)
                        .map(String::from),
                    items: Self::keys(k.get("items")),
                })
            })
            .collect()
    }

    // ------------------------------------------------------------ Validierung

    /// Alle Verstöße gegen das Schema; leer = gültig.
    pub fn validate(&self) -> Vec<String> {
        let mut e = Vec::new();
        let root = &self.root;
        let req =
            |e: &mut Vec<String>, path: &str, what: &str, ok: &dyn Fn(&str) -> bool| match root
                .at(path)
                .and_then(Json::as_str)
            {
                None => e.push(format!("{path}: fehlt (String erwartet)")),
                Some(v) if !ok(v) => e.push(format!("{path}: {what}, ist '{v}'")),
                _ => {}
            };
        req(
            &mut e,
            "meta.schema",
            &format!("muss '{SCHEMA}' sein"),
            &|v| v == SCHEMA,
        );
        for p in [
            "meta.profile_id",
            "meta.source.title",
            "meta.source.manufacturer",
            "meta.source.part_number",
            "meta.source.document_version",
            "meta.source.file_name",
            "meta.capture.extracted_by",
        ] {
            req(&mut e, p, "darf nicht leer sein", &|v| !v.trim().is_empty());
        }
        req(
            &mut e,
            "meta.source.url",
            "muss mit https:// beginnen",
            &|v| v.starts_with("https://") && v.len() > 12,
        );
        req(
            &mut e,
            "meta.source.file_sha256",
            "muss 64 Kleinbuchstaben-Hexziffern sein",
            &|v| {
                v.len() == 64
                    && v.bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            },
        );
        req(
            &mut e,
            "meta.source.published",
            "muss YYYY-MM-DD sein",
            &is_date,
        );
        req(
            &mut e,
            "meta.capture.extracted_at",
            "muss YYYY-MM-DDTHH:MM:SSZ (UTC) sein",
            &is_timestamp,
        );
        req(
            &mut e,
            "meta.capture.review_status",
            "muss 'unreviewed' oder 'reviewed' sein",
            &|v| matches!(v, "unreviewed" | "reviewed"),
        );
        if self
            .root
            .at("meta.capture.verified_against_device")
            .and_then(Json::as_bool)
            .is_none()
        {
            e.push("meta.capture.verified_against_device: fehlt (true/false erwartet)".into());
        }
        if self.s("meta.capture.review_status") == "reviewed" {
            for p in ["meta.capture.reviewed_by", "meta.capture.reviewed_at"] {
                if self
                    .root
                    .at(p)
                    .and_then(Json::as_str)
                    .is_none_or(|v| v.is_empty())
                {
                    e.push(format!("{p}: bei review_status 'reviewed' Pflicht"));
                }
            }
        }

        // UIDs
        fn uid_ok(e: &mut Vec<String>, path: String, uid: &str) {
            if !is_uid(uid) {
                e.push(format!("{path}: '{uid}' ist keine gültige UID"));
            }
        }
        for (i, c) in self.sop_classes().iter().enumerate() {
            uid_ok(&mut e, format!("sop_classes[{i}].uid"), &c.uid);
        }
        for (i, c) in self.storage_contexts().iter().enumerate() {
            uid_ok(
                &mut e,
                format!("storage.proposed_contexts[{i}].uid"),
                &c.uid,
            );
            let mut seen = Vec::new();
            for (j, t) in c.transfer_syntaxes.iter().enumerate() {
                uid_ok(
                    &mut e,
                    format!("storage.proposed_contexts[{i}].transfer_syntaxes[{j}].uid"),
                    &t.uid,
                );
                if seen.contains(&t.uid) {
                    e.push(format!(
                        "storage.proposed_contexts[{i}]: Transfer Syntax {} doppelt",
                        t.uid
                    ));
                }
                seen.push(t.uid.clone());
            }
            if c.transfer_syntaxes.is_empty() {
                e.push(format!(
                    "storage.proposed_contexts[{i}]: keine Transfer Syntax"
                ));
            }
        }
        for (j, t) in self.worklist_transfer_syntaxes().iter().enumerate() {
            uid_ok(
                &mut e,
                format!("worklist.proposed_transfer_syntaxes[{j}].uid"),
                &t.uid,
            );
        }
        if self.worklist_transfer_syntaxes().is_empty() {
            e.push("worklist.proposed_transfer_syntaxes: leer".into());
        }

        // Worklist-Schlüssel
        match self.root.at("worklist.keys").and_then(Json::as_arr) {
            None => e.push("worklist.keys: fehlt (Array erwartet)".into()),
            Some(keys) => validate_keys(keys, "worklist.keys", &mut e),
        }
        e
    }
}

fn validate_keys(keys: &[Json], path: &str, e: &mut Vec<String>) {
    let mut seen: Vec<&str> = Vec::new();
    for (i, k) in keys.iter().enumerate() {
        let p = format!("{path}[{i}]");
        let tag = k.get("tag").and_then(Json::as_str).unwrap_or("");
        if parse_tag(tag).is_none() {
            e.push(format!(
                "{p}.tag: '{tag}' ist kein Tag (GGGG,EEEE in Großbuchstaben-Hex)"
            ));
        } else if seen.contains(&tag) {
            e.push(format!("{p}.tag: {tag} kommt auf dieser Ebene doppelt vor"));
        }
        seen.push(tag);
        let vr = k.get("vr").and_then(Json::as_str).unwrap_or("");
        if !VRS.contains(&vr) {
            e.push(format!("{p}.vr: '{vr}' ist kein bekanntes VR"));
        }
        if k.get("name")
            .and_then(Json::as_str)
            .is_none_or(str::is_empty)
        {
            e.push(format!("{p}.name: fehlt"));
        }
        match k.get("items") {
            Some(items) if vr != "SQ" => {
                let _ = items;
                e.push(format!("{p}.items: nur bei VR SQ erlaubt"));
            }
            Some(items) => match items.as_arr() {
                Some(a) => validate_keys(a, &format!("{p}.items"), e),
                None => e.push(format!("{p}.items: Array erwartet")),
            },
            None => {}
        }
    }
}

/// `GGGG,EEEE` → (Gruppe, Element); nur Großbuchstaben-Hex.
pub fn parse_tag(s: &str) -> Option<(u16, u16)> {
    let (g, el) = s.split_once(',')?;
    let ok = |h: &str| {
        h.len() == 4
            && h.bytes()
                .all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(&b))
    };
    if !ok(g) || !ok(el) {
        return None;
    }
    Some((
        u16::from_str_radix(g, 16).ok()?,
        u16::from_str_radix(el, 16).ok()?,
    ))
}

fn is_uid(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.split('.').all(|c| {
            !c.is_empty()
                && c.bytes().all(|b| b.is_ascii_digit())
                && (c == "0" || !c.starts_with('0'))
        })
        && s.contains('.')
}

fn is_date(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    let n = |r: std::ops::Range<usize>| s.get(r).and_then(|x| x.parse::<u32>().ok());
    matches!((n(0..4), n(5..7), n(8..10)), (Some(y), Some(m), Some(d)) if y >= 1990 && (1..=12).contains(&m) && (1..=31).contains(&d))
}

fn is_timestamp(s: &str) -> bool {
    if s.len() != 20 || !s.ends_with('Z') || s.as_bytes()[10] != b'T' {
        return false;
    }
    let t = &s[11..19];
    let n = |r: std::ops::Range<usize>| t.get(r).and_then(|x| x.parse::<u32>().ok());
    is_date(&s[..10])
        && t.as_bytes()[2] == b':'
        && t.as_bytes()[5] == b':'
        && matches!((n(0..2), n(3..5), n(6..8)), (Some(h), Some(m), Some(sec)) if h < 24 && m < 60 && sec < 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Das eingecheckte Mindray-Profil.
    pub const MINDRAY: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../devices/mindray-dp/dicom-conformance.json"
    );

    fn text() -> String {
        std::fs::read_to_string(MINDRAY).unwrap()
    }

    fn expect_error(mutated: String, needle: &str) {
        let errs = Profile::parse(&mutated)
            .err()
            .unwrap_or_else(|| panic!("hätte scheitern müssen ({needle})"));
        assert!(
            errs.iter().any(|e| e.contains(needle)),
            "erwartet '{needle}' in {errs:#?}"
        );
    }

    #[test]
    fn committed_mindray_profile_is_valid() {
        let p = Profile::load(MINDRAY).unwrap_or_else(|e| panic!("{e:#?}"));
        let m = p.meta();
        assert_eq!(m.profile_id, "mindray-dp");
        assert!(m.source_url.contains("mindray.com"));
        assert_eq!(m.file_sha256.len(), 64);
        assert!(!p.worklist_keys().is_empty());
        assert!(!p.storage_contexts().is_empty());
        assert!(p.find_sop_class("worklist").is_some());
    }

    #[test]
    fn typed_view_exposes_nested_keys() {
        let p = Profile::load(MINDRAY).unwrap();
        let sps = p
            .worklist_keys()
            .into_iter()
            .find(|k| k.tag == (0x0040, 0x0100))
            .unwrap();
        assert_eq!(&sps.vr, b"SQ");
        let modality = sps
            .items
            .iter()
            .find(|k| k.tag == (0x0008, 0x0060))
            .unwrap();
        assert_eq!(modality.fixed_value.as_deref(), Some("US"));
        let code = sps
            .items
            .iter()
            .find(|k| k.tag == (0x0040, 0x0008))
            .unwrap();
        assert!(!code.items.is_empty(), "Sequenz in Sequenz");
    }

    #[test]
    fn rejects_missing_or_bad_metadata() {
        expect_error(
            text().replace("\"url\": \"https://", "\"url\": \"http://"),
            "meta.source.url",
        );
        expect_error(
            text().replacen("\"file_sha256\": \"", "\"file_sha256\": \"zz", 1),
            "file_sha256",
        );
        expect_error(
            text().replace("\"unreviewed\"", "\"vielleicht\""),
            "review_status",
        );
        expect_error(
            text().replace("\"opengdt-device-profile/1\"", "\"anderes/9\""),
            "meta.schema",
        );
        let m = Profile::load(MINDRAY).unwrap().meta();
        expect_error(
            text().replace(&m.extracted_at, "15.10.2026"),
            "extracted_at",
        );
        expect_error(
            text().replace(&m.extracted_at, "2026-13-40T25:61:61Z"),
            "extracted_at",
        );
        expect_error(text().replace(&m.published, "2011-1-13"), "published");
    }

    #[test]
    fn reviewed_requires_reviewer() {
        expect_error(
            text().replace("\"unreviewed\"", "\"reviewed\""),
            "reviewed_by",
        );
    }

    #[test]
    fn rejects_bad_data_rows() {
        expect_error(
            text().replacen("\"tag\": \"0010,0010\"", "\"tag\": \"0010,001\"", 1),
            "kein Tag",
        );
        expect_error(
            text().replacen("\"tag\": \"0010,0010\"", "\"tag\": \"0010,001g\"", 1),
            "kein Tag",
        );
        expect_error(
            text().replacen("\"vr\": \"PN\"", "\"vr\": \"XX\"", 1),
            "kein bekanntes VR",
        );
        expect_error(
            text().replacen("1.2.840.10008.5.1.4.31", "1.2.840.10008.5.1.4.031", 1),
            "keine gültige UID",
        );
        expect_error(
            text().replacen("1.2.840.10008.1.2.4.50", "abc", 1),
            "keine gültige UID",
        );
    }

    #[test]
    fn rejects_broken_json_with_position() {
        let errs = Profile::parse("{\"meta\": }").err().unwrap();
        assert!(errs[0].contains("Zeile 1"), "{errs:?}");
    }

    #[test]
    fn tag_parser() {
        assert_eq!(parse_tag("7FE0,0010"), Some((0x7FE0, 0x0010)));
        assert_eq!(
            parse_tag("7fe0,0010"),
            None,
            "nur Großbuchstaben, damit Diffs stabil bleiben"
        );
        assert_eq!(parse_tag("0010 0010"), None);
    }
}
