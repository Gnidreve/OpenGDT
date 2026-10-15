//! Zeichensätze für GDT-Dateien (Feld 9206).

use crate::Error;

const CP437_HIGH: [char; 128] = [
    '\u{00c7}', '\u{00fc}', '\u{00e9}', '\u{00e2}', '\u{00e4}', '\u{00e0}', '\u{00e5}', '\u{00e7}',
    '\u{00ea}', '\u{00eb}', '\u{00e8}', '\u{00ef}', '\u{00ee}', '\u{00ec}', '\u{00c4}', '\u{00c5}',
    '\u{00c9}', '\u{00e6}', '\u{00c6}', '\u{00f4}', '\u{00f6}', '\u{00f2}', '\u{00fb}', '\u{00f9}',
    '\u{00ff}', '\u{00d6}', '\u{00dc}', '\u{00a2}', '\u{00a3}', '\u{00a5}', '\u{20a7}', '\u{0192}',
    '\u{00e1}', '\u{00ed}', '\u{00f3}', '\u{00fa}', '\u{00f1}', '\u{00d1}', '\u{00aa}', '\u{00ba}',
    '\u{00bf}', '\u{2310}', '\u{00ac}', '\u{00bd}', '\u{00bc}', '\u{00a1}', '\u{00ab}', '\u{00bb}',
    '\u{2591}', '\u{2592}', '\u{2593}', '\u{2502}', '\u{2524}', '\u{2561}', '\u{2562}', '\u{2556}',
    '\u{2555}', '\u{2563}', '\u{2551}', '\u{2557}', '\u{255d}', '\u{255c}', '\u{255b}', '\u{2510}',
    '\u{2514}', '\u{2534}', '\u{252c}', '\u{251c}', '\u{2500}', '\u{253c}', '\u{255e}', '\u{255f}',
    '\u{255a}', '\u{2554}', '\u{2569}', '\u{2566}', '\u{2560}', '\u{2550}', '\u{256c}', '\u{2567}',
    '\u{2568}', '\u{2564}', '\u{2565}', '\u{2559}', '\u{2558}', '\u{2552}', '\u{2553}', '\u{256b}',
    '\u{256a}', '\u{2518}', '\u{250c}', '\u{2588}', '\u{2584}', '\u{258c}', '\u{2590}', '\u{2580}',
    '\u{03b1}', '\u{00df}', '\u{0393}', '\u{03c0}', '\u{03a3}', '\u{03c3}', '\u{00b5}', '\u{03c4}',
    '\u{03a6}', '\u{0398}', '\u{03a9}', '\u{03b4}', '\u{221e}', '\u{03c6}', '\u{03b5}', '\u{2229}',
    '\u{2261}', '\u{00b1}', '\u{2265}', '\u{2264}', '\u{2320}', '\u{2321}', '\u{00f7}', '\u{2248}',
    '\u{00b0}', '\u{2219}', '\u{00b7}', '\u{221a}', '\u{207f}', '\u{00b2}', '\u{25a0}', '\u{00a0}',
];

/// Zeichensatz einer GDT-Datei.
///
/// Wert aus Feld 9206: 1 = 7-Bit, 2 = IBM-PC (CP437), 3 = ISO 8859-1.
/// `Utf8` ist keine Norm-Angabe, sondern eine Erweiterung, die manche PVS
/// tatsächlich schreiben; sie kann nur per Konfiguration gewählt werden.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Charset {
    SevenBit,
    Cp437,
    Latin1,
    Utf8,
}

impl Charset {
    /// Wert für Feld 9206 (`Utf8` hat keinen Normwert und wird als 3 geschrieben,
    /// sofern der Aufrufer es nicht anders behandelt).
    pub fn field_value(self) -> &'static str {
        match self {
            Charset::SevenBit => "1",
            Charset::Cp437 => "2",
            Charset::Latin1 | Charset::Utf8 => "3",
        }
    }

    pub fn from_field_value(v: &str) -> Option<Charset> {
        match v.trim() {
            "1" => Some(Charset::SevenBit),
            "2" => Some(Charset::Cp437),
            "3" => Some(Charset::Latin1),
            _ => None,
        }
    }

    pub fn decode(self, bytes: &[u8]) -> Result<String, Error> {
        match self {
            Charset::Utf8 => String::from_utf8(bytes.to_vec()).map_err(|_| Error::Encoding),
            Charset::Latin1 => Ok(bytes.iter().map(|&b| b as char).collect()),
            Charset::SevenBit => {
                if bytes.iter().all(|b| b.is_ascii()) {
                    Ok(bytes.iter().map(|&b| b as char).collect())
                } else {
                    Err(Error::Encoding)
                }
            }
            Charset::Cp437 => Ok(bytes
                .iter()
                .map(|&b| {
                    if b < 128 {
                        b as char
                    } else {
                        CP437_HIGH[(b - 128) as usize]
                    }
                })
                .collect()),
        }
    }

    pub fn encode(self, s: &str) -> Result<Vec<u8>, Error> {
        match self {
            Charset::Utf8 => Ok(s.as_bytes().to_vec()),
            Charset::SevenBit => s
                .chars()
                .map(|c| {
                    if c.is_ascii() {
                        Ok(c as u8)
                    } else {
                        Err(Error::Encoding)
                    }
                })
                .collect(),
            Charset::Latin1 => s
                .chars()
                .map(|c| u8::try_from(c as u32).map_err(|_| Error::Encoding))
                .collect(),
            Charset::Cp437 => s
                .chars()
                .map(|c| {
                    if c.is_ascii() {
                        Ok(c as u8)
                    } else {
                        CP437_HIGH
                            .iter()
                            .position(|&h| h == c)
                            .map(|i| 128 + i as u8)
                            .ok_or(Error::Encoding)
                    }
                })
                .collect(),
        }
    }
}
