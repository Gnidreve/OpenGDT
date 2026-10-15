//! DICOM-Schlüsselvergleich für C-FIND (PS3.4 C.2.2.2) und Antwort-Projektion.

use opengdt_dicom::{Dataset, Element, Value, Vr};

#[derive(Clone, Copy)]
pub struct MatchOptions {
    /// Datumsfelder (DA) beim Matching berücksichtigen. Standard: aus –
    /// siehe README (Zeitzonenproblem).
    pub match_dates: bool,
}

/// Prüft, ob `entry` zu den Suchschlüsseln in `query` passt.
pub fn matches(entry: &Dataset, query: &Dataset, o: MatchOptions) -> bool {
    query.iter().all(|key| match &key.value {
        // Sequenz, die nur aus leeren Schlüsseln besteht, ist ein reiner Return Key
        // (z. B. „Referenced Study Sequence“ mit leeren UIDs): trifft immer, auch wenn
        // der Eintrag die Sequenz gar nicht hat.
        Value::Sequence(items) if items.iter().all(is_universal) => true,
        Value::Sequence(items) => match entry.get(key.tag).map(|e| &e.value) {
            // Es muss ein gespeichertes Item zu allen Query-Items passen.
            Some(Value::Sequence(stored)) => stored
                .iter()
                .any(|s| items.iter().all(|q| matches(s, q, o))),
            _ => false,
        },
        Value::Fragments(_) => true,
        Value::Bytes(b) => {
            let k = text(b);
            if k.is_empty() {
                return true; // universal matching
            }
            if key.vr == Vr::DA && !o.match_dates {
                return true;
            }
            let stored = entry.get_str(key.tag).unwrap_or_default();
            if stored.is_empty() {
                // Gespeicherter Wert leer: nur universelles Matching trifft.
                return false;
            }
            match key.vr {
                Vr::DA => date_matches(&k, &stored),
                Vr::PN | Vr::LO | Vr::SH | Vr::AE | Vr::CS => {
                    wildcard(&k, &stored, key.vr == Vr::PN)
                }
                _ => k == stored,
            }
        }
    })
}

/// Item ohne einen einzigen echten Suchwert (alle Werte leer, Sequenzen rekursiv ebenso).
fn is_universal(item: &Dataset) -> bool {
    item.iter().all(|e| match &e.value {
        Value::Bytes(b) => text(b).is_empty(),
        Value::Sequence(items) => items.iter().all(is_universal),
        Value::Fragments(_) => true,
    })
}

fn text(b: &[u8]) -> String {
    let s: String = b.iter().map(|&c| c as char).collect();
    s.trim_end_matches(['\0', ' ']).to_string()
}

/// `*` und `?` Wildcards; PN und Strings werden ohne Beachtung der Groß-/Kleinschreibung
/// verglichen (PN laut Standard, Rest bewusst großzügig).
fn wildcard(pattern: &str, value: &str, _is_pn: bool) -> bool {
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let v: Vec<char> = value.to_lowercase().chars().collect();
    fn rec(p: &[char], v: &[char]) -> bool {
        match p.first() {
            None => v.is_empty(),
            Some('*') => (0..=v.len()).any(|i| rec(&p[1..], &v[i..])),
            Some('?') => !v.is_empty() && rec(&p[1..], &v[1..]),
            Some(c) => v.first() == Some(c) && rec(&p[1..], &v[1..]),
        }
    }
    rec(&p, &v)
}

/// Einzeldatum oder Bereich `YYYYMMDD-YYYYMMDD` (offene Enden erlaubt).
fn date_matches(key: &str, value: &str) -> bool {
    match key.split_once('-') {
        None => key == value,
        Some((from, to)) => (from.is_empty() || value >= from) && (to.is_empty() || value <= to),
    }
}

/// Antwort-Datensatz: nur die angefragten Attribute (rekursiv für Sequenzen),
/// plus SpecificCharacterSet, damit Textkodierung eindeutig bleibt.
pub fn project(entry: &Dataset, query: &Dataset) -> Dataset {
    let mut out = Dataset::new();
    if let Some(cs) = entry.get(opengdt_dicom::t::SPECIFIC_CHARACTER_SET) {
        out.insert(cs.clone());
    }
    for key in query.iter() {
        let Some(stored) = entry.get(key.tag) else {
            continue;
        };
        match (&key.value, &stored.value) {
            (Value::Sequence(q), Value::Sequence(s)) => {
                let items: Vec<Dataset> = s
                    .iter()
                    .map(|item| match q.first() {
                        Some(qi) if !qi.is_empty() => project(item, qi),
                        _ => item.clone(),
                    })
                    .collect();
                out.insert(Element {
                    tag: key.tag,
                    vr: Vr::SQ,
                    value: Value::Sequence(items),
                });
            }
            _ => out.insert(stored.clone()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use opengdt_dicom::t;

    fn entry() -> Dataset {
        let mut sps = Dataset::new();
        sps.set_str(t::MODALITY, "US");
        sps.set_str(t::SPS_START_DATE, "20261015");
        sps.set_str(t::SPS_STATION_AE, "MINDRAY");
        let mut e = Dataset::new();
        e.set_str(t::PATIENT_NAME, "Müller^Jürgen");
        e.set_str(t::PATIENT_ID, "4711");
        e.set_seq(t::SPS_SEQUENCE, vec![sps]);
        e
    }

    const O: MatchOptions = MatchOptions { match_dates: true };

    #[test]
    fn empty_query_matches_everything() {
        assert!(matches(&entry(), &Dataset::new(), O));
        let mut q = Dataset::new();
        q.set_str(t::PATIENT_NAME, "");
        assert!(matches(&entry(), &q, O));
    }

    #[test]
    fn wildcards_and_case() {
        let mut q = Dataset::new();
        q.set_str(t::PATIENT_NAME, "m*^J?rgen");
        assert!(matches(&entry(), &q, O));
        q.set_str(t::PATIENT_NAME, "Meier*");
        assert!(!matches(&entry(), &q, O));
    }

    #[test]
    fn sequence_keys_and_dates() {
        let mut item = Dataset::new();
        item.set_str(t::MODALITY, "US");
        item.set_str(t::SPS_START_DATE, "20261014-20261016");
        let mut q = Dataset::new();
        q.set_seq(t::SPS_SEQUENCE, vec![item.clone()]);
        assert!(matches(&entry(), &q, O));
        item.set_str(t::MODALITY, "CT");
        q.set_seq(t::SPS_SEQUENCE, vec![item.clone()]);
        assert!(!matches(&entry(), &q, O));
        item.set_str(t::MODALITY, "US");
        item.set_str(t::SPS_START_DATE, "20270101-");
        q.set_seq(t::SPS_SEQUENCE, vec![item]);
        assert!(!matches(&entry(), &q, O));
        assert!(matches(&entry(), &q, MatchOptions { match_dates: false }));
    }

    #[test]
    fn return_key_sequences_match_even_if_entry_lacks_them() {
        // Wie beim Mindray DP-70/DP-50: Sequenzen mit leeren Unter-Schlüsseln.
        let mut child = Dataset::new();
        child.set_str(t::SOP_CLASS_UID, "");
        child.set_str(t::SOP_INSTANCE_UID, "");
        let mut q = Dataset::new();
        q.set_seq(opengdt_dicom::Tag(0x0008, 0x1110), vec![child]); // fehlt im Eintrag
        q.set_seq(opengdt_dicom::Tag(0x0008, 0x1120), vec![Dataset::new()]);
        q.set_seq(opengdt_dicom::Tag(0x0008, 0x1111), vec![]);
        assert!(matches(&entry(), &q, O));
        // Ein echter Suchwert in der fehlenden Sequenz darf dagegen nicht treffen.
        let mut real = Dataset::new();
        real.set_str(t::SOP_INSTANCE_UID, "1.2.3");
        let mut q2 = Dataset::new();
        q2.set_seq(opengdt_dicom::Tag(0x0008, 0x1110), vec![real]);
        assert!(!matches(&entry(), &q2, O));
    }

    #[test]
    fn projection_only_returns_requested() {
        let mut item = Dataset::new();
        item.set_str(t::SPS_STATION_AE, "");
        let mut q = Dataset::new();
        q.set_str(t::PATIENT_ID, "");
        q.set_seq(t::SPS_SEQUENCE, vec![item]);
        let r = project(&entry(), &q);
        assert_eq!(r.get_str(t::PATIENT_ID).as_deref(), Some("4711"));
        assert!(r.get(t::PATIENT_NAME).is_none());
        let sps = &r.get_seq(t::SPS_SEQUENCE).unwrap()[0];
        assert_eq!(sps.get_str(t::SPS_STATION_AE).as_deref(), Some("MINDRAY"));
        assert!(sps.get(t::MODALITY).is_none());
    }
}
