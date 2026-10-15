//! Fuzz-Rauchtest: gültige Eingaben werden zufällig verfälscht (deterministisch, ohne externe Hilfsmittel).
//! Keine Eingabe darf einen Panic auslösen – die Decoder hängen am Netzwerk.

use opengdt_dicom::dataset::{Element, Value};
use opengdt_dicom::file::{read_part10, write_part10};
use opengdt_dicom::pdu::{
    read_pdu, write_pdu, Pdu, Pdv, PresentationContextAc, PresentationContextRq,
};
use opengdt_dicom::{t, uid, Dataset, Tag, Vr};
use std::panic::catch_unwind;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

/// Zufällige Verfälschung: Bytes ändern, löschen, einfügen, abschneiden, Blöcke kopieren.
fn mutate(rng: &mut Rng, src: &[u8]) -> Vec<u8> {
    let mut v = src.to_vec();
    for _ in 0..1 + rng.below(6) {
        if v.is_empty() {
            v.push(rng.next() as u8);
            continue;
        }
        match rng.below(6) {
            0 => {
                let i = rng.below(v.len());
                v[i] = rng.next() as u8;
            }
            1 => {
                let i = rng.below(v.len());
                v.remove(i);
            }
            2 => {
                let i = rng.below(v.len() + 1);
                v.insert(i, rng.next() as u8);
            }
            3 => v.truncate(rng.below(v.len() + 1)),
            4 => {
                // 0xFF-Läufe (undefinierte Längen) und große Längenfelder erzeugen
                let i = rng.below(v.len());
                for b in v.iter_mut().skip(i).take(1 + rng.below(4)) {
                    *b = 0xFF;
                }
            }
            _ => {
                let a = rng.below(v.len());
                let b = a + rng.below(v.len() - a + 1);
                let block = v[a..b].to_vec();
                let at = rng.below(v.len() + 1);
                v.splice(at..at, block);
            }
        }
    }
    v
}

fn sample_dataset() -> Dataset {
    let mut item = Dataset::new();
    item.set_str(t::MODALITY, "US");
    item.set_str(t::SPS_STATION_AE, "MINDRAY");
    let mut inner = Dataset::new();
    inner.set_seq(t::SPS_SEQUENCE, vec![item.clone()]);
    let mut ds = Dataset::new();
    ds.set_str(t::PATIENT_NAME, "Müller^Jürgen");
    ds.set_u16(t::ROWS, 480);
    ds.set_seq(t::SPS_SEQUENCE, vec![item, inner]);
    ds.insert(Element {
        tag: t::PIXEL_DATA,
        vr: Vr::OB,
        value: Value::Fragments(vec![vec![], vec![1, 2, 3, 4], vec![5, 6]]),
    });
    ds.set_bytes(Tag(0x0010, 0x21B0), Vr::UN, vec![1, 2, 3]);
    ds
}

fn pdus() -> Vec<Vec<u8>> {
    let rq = Pdu::AssociateRq {
        called_ae: "OPENGDT".into(),
        calling_ae: "DP50".into(),
        contexts: vec![PresentationContextRq {
            id: 1,
            abstract_syntax: uid::VERIFICATION.into(),
            transfer_syntaxes: vec![uid::IMPLICIT_VR_LE.into(), uid::EXPLICIT_VR_LE.into()],
        }],
        max_pdu: 16384,
    };
    let ac = Pdu::AssociateAc {
        called_ae: "A".into(),
        calling_ae: "B".into(),
        contexts: vec![PresentationContextAc {
            id: 1,
            result: 0,
            transfer_syntax: uid::IMPLICIT_VR_LE.into(),
        }],
        max_pdu: 16384,
    };
    let data = Pdu::Data(vec![
        Pdv {
            context_id: 1,
            is_command: true,
            is_last: true,
            data: vec![1, 2, 3, 4, 5],
        },
        Pdv {
            context_id: 1,
            is_command: false,
            is_last: false,
            data: vec![9; 40],
        },
    ]);
    [
        rq,
        ac,
        data,
        Pdu::ReleaseRq,
        Pdu::Abort {
            source: 1,
            reason: 2,
        },
    ]
    .iter()
    .map(|p| {
        let mut b = Vec::new();
        write_pdu(&mut b, p).unwrap();
        b
    })
    .collect()
}

fn run(
    name: &str,
    seeds: &[Vec<u8>],
    iterations: usize,
    f: impl Fn(&[u8]) + std::panic::RefUnwindSafe,
) {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15 ^ name.len() as u64);
    let mut panics = Vec::new();
    for i in 0..iterations {
        let seed = &seeds[rng.below(seeds.len())];
        let input = mutate(&mut rng, seed);
        if catch_unwind(|| f(&input)).is_err() {
            panics.push((i, input));
            if panics.len() >= 3 {
                break;
            }
        }
    }
    assert!(
        panics.is_empty(),
        "{name}: Panic bei {} Eingaben, erste: {:02X?}",
        panics.len(),
        panics[0].1
    );
}

#[test]
fn dataset_decoder_survives_garbage() {
    let ds = sample_dataset();
    let seeds = vec![ds.encode(true), ds.encode(false)];
    run("dataset-explicit", &seeds, 40_000, |b| {
        let _ = Dataset::decode(b, true);
    });
    run("dataset-implicit", &seeds, 40_000, |b| {
        let _ = Dataset::decode(b, false);
    });
}

#[test]
fn pdu_reader_survives_garbage() {
    run("pdu", &pdus(), 60_000, |b| {
        let _ = read_pdu(&mut &b[..]);
    });
}

#[test]
fn part10_reader_survives_garbage() {
    let file = write_part10(
        "1.2.840.10008.5.1.4.1.1.6.1",
        "1.2.3",
        uid::EXPLICIT_VR_LE,
        &sample_dataset().encode(true),
    );
    run("part10", &[file], 40_000, |b| {
        let _ = read_part10(b);
    });
}
