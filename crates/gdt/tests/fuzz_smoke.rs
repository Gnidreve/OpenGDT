//! Fuzz-Rauchtest für GDT-Dateien: beliebige Bytes dürfen nie einen Panic auslösen.

use opengdt_gdt::{Charset, Patient, Record};
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

fn mutate(rng: &mut Rng, src: &[u8]) -> Vec<u8> {
    let mut v = src.to_vec();
    for _ in 0..1 + rng.below(8) {
        if v.is_empty() {
            v.push(rng.next() as u8);
            continue;
        }
        match rng.below(5) {
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
                v.insert(
                    i,
                    *b"\r\n0123456789\xC4\xD6\xFF"
                        .get(rng.below(15))
                        .unwrap_or(&b'x'),
                );
            }
            3 => v.truncate(rng.below(v.len() + 1)),
            _ => {
                let a = rng.below(v.len());
                let b = a + rng.below(v.len() - a + 1);
                let block = v[a..b].to_vec();
                v.splice(a..a, block);
            }
        }
    }
    v
}

#[test]
fn gdt_parser_and_patient_mapping_survive_garbage() {
    let sample = b"01380006301\r\n0143000 4711\r\n0153101Mueller\r\n017310307031965\r\n01031101\r\n0109206\x33\r\n".to_vec();
    let mut rng = Rng(0xABCD_EF01_2345_6789);
    for i in 0..60_000 {
        let input = mutate(&mut rng, &sample);
        for strict in [false, true] {
            let r = catch_unwind(|| {
                if let Ok(rec) = Record::parse(&input, strict) {
                    let _ = Patient::from_record(&rec);
                    let _ = rec.charset();
                    let _ = rec.to_bytes();
                }
            });
            assert!(
                r.is_ok(),
                "Panic bei Durchlauf {i} (strict={strict}): {input:02X?}"
            );
        }
    }
    // alle Zeichensätze gegen beliebige Bytes
    for cs in [
        Charset::SevenBit,
        Charset::Cp437,
        Charset::Latin1,
        Charset::Utf8,
    ] {
        for _ in 0..2000 {
            let n = rng.below(20);
            let bytes: Vec<u8> = (0..n).map(|_| rng.next() as u8).collect();
            let _ = cs.decode(&bytes);
        }
    }
}
