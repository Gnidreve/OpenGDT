//! Fuzz-Rauchtest für den JSON-Parser (Eingaben kommen auch aus dem Netzwerk, z. B. von Orthanc).

use opengdt_json::Json;
use std::panic::catch_unwind;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

#[test]
fn json_parser_survives_garbage() {
    let seed = r#"{"Changes":[{"ChangeType":"StableStudy","ID":"a-b","Seq":7,"x":[1,-2.5e3,true,null,"ü😀"]}],"Done":false,"Last":99}"#;
    let alphabet: Vec<char> = "{}[]\",:\\-+.eE0123456789tfnul \n\u{e4}".chars().collect();
    let mut rng = Rng(0x0F0F_1234_ABCD_9876);
    for i in 0..80_000 {
        let mut chars: Vec<char> = seed.chars().collect();
        for _ in 0..1 + (rng.next() % 5) {
            let at = (rng.next() % (chars.len() as u64 + 1)) as usize;
            match rng.next() % 3 {
                0 if at < chars.len() => {
                    chars[at] = alphabet[(rng.next() % alphabet.len() as u64) as usize]
                }
                1 if at < chars.len() => {
                    chars.remove(at);
                }
                _ => chars.insert(at, alphabet[(rng.next() % alphabet.len() as u64) as usize]),
            }
        }
        let text: String = chars.into_iter().collect();
        assert!(
            catch_unwind(|| {
                let _ = Json::parse(&text);
            })
            .is_ok(),
            "Panic bei Durchlauf {i}: {text:?}"
        );
    }
}
