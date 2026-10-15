use crate::{ids, Charset, Error};

/// Eine GDT-Zeile: Feldkennung + Rohbytes (noch nicht dekodiert).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    pub id: u16,
    pub value: Vec<u8>,
}

/// Ein kompletter GDT-Satz (Datei).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub fields: Vec<Field>,
}

impl Record {
    /// Parst GDT-Bytes.
    ///
    /// `strict`: Zeilenlängen (`LLL`) müssen exakt stimmen. Im lenienten Modus
    /// wird nur an Zeilenenden getrennt – in der Praxis liefern PVS gelegentlich
    /// falsche Längen, die Feld 8100 ist ohnehin nie zuverlässig.
    pub fn parse(data: &[u8], strict: bool) -> Result<Record, Error> {
        let mut fields = Vec::new();
        for (idx, raw) in data.split(|&b| b == b'\n').enumerate() {
            let line_no = idx + 1;
            let had_cr = raw.last() == Some(&b'\r');
            let line = if had_cr { &raw[..raw.len() - 1] } else { raw };
            if line.is_empty() {
                continue;
            }
            if line.len() < 7 {
                return Err(Error::MalformedLine(line_no));
            }
            let declared = ascii_num(&line[..3]).ok_or(Error::MalformedLine(line_no))?;
            let id = ascii_num(&line[3..7]).ok_or(Error::MalformedLine(line_no))? as u16;
            if strict {
                let actual = line.len() + 2; // CRLF
                if declared != actual {
                    return Err(Error::LengthMismatch {
                        line: line_no,
                        declared,
                        actual,
                    });
                }
            }
            fields.push(Field {
                id,
                value: line[7..].to_vec(),
            });
        }
        Ok(Record { fields })
    }

    /// Zeichensatz laut Feld 9206; ohne Angabe Latin-1 (häufigster Praxisfall).
    pub fn charset(&self) -> Charset {
        self.raw(ids::CHARSET)
            .and_then(|v| std::str::from_utf8(v).ok())
            .and_then(Charset::from_field_value)
            .unwrap_or(Charset::Latin1)
    }

    pub fn raw(&self, id: u16) -> Option<&[u8]> {
        self.fields
            .iter()
            .find(|f| f.id == id)
            .map(|f| f.value.as_slice())
    }

    /// Erstes Feld mit dieser Kennung, dekodiert.
    pub fn get(&self, id: u16, cs: Charset) -> Result<Option<String>, Error> {
        self.raw(id).map(|v| cs.decode(v)).transpose()
    }

    pub fn require(&self, id: u16, cs: Charset) -> Result<String, Error> {
        self.get(id, cs)?.ok_or(Error::MissingField(id))
    }

    /// Serialisiert den Satz. Feld 8000 muss als erstes Feld gesetzt sein;
    /// Feld 8100 (Satzlänge) wird automatisch berechnet und direkt danach eingefügt.
    pub fn to_bytes(&self) -> Vec<u8> {
        let lines: Vec<Vec<u8>> = self
            .fields
            .iter()
            .filter(|f| f.id != ids::RECORD_LENGTH)
            .map(|f| line(f.id, &f.value))
            .collect();
        let body: usize = lines.iter().map(|l| l.len()).sum();
        // 8100-Zeile: 3 + 4 + 5 + 2 = 14 Bytes
        let total = body + 14;
        let mut out = Vec::with_capacity(total);
        for (i, l) in lines.iter().enumerate() {
            out.extend_from_slice(l);
            if i == 0 {
                out.extend_from_slice(&line(ids::RECORD_LENGTH, format!("{total:05}").as_bytes()));
            }
        }
        out
    }
}

fn line(id: u16, value: &[u8]) -> Vec<u8> {
    let len = 3 + 4 + value.len() + 2;
    let mut l = format!("{len:03}{id:04}").into_bytes();
    l.extend_from_slice(value);
    l.extend_from_slice(b"\r\n");
    l
}

fn ascii_num(b: &[u8]) -> Option<usize> {
    if b.is_empty() || !b.iter().all(|c| c.is_ascii_digit()) {
        return None;
    }
    std::str::from_utf8(b).ok()?.parse().ok()
}

/// Hilfsbuilder für das Schreiben.
pub(crate) struct Builder {
    pub cs: Charset,
    pub fields: Vec<Field>,
}

impl Builder {
    pub fn new(cs: Charset) -> Self {
        Builder {
            cs,
            fields: Vec::new(),
        }
    }
    pub fn add(&mut self, id: u16, value: &str) -> Result<(), Error> {
        self.fields.push(Field {
            id,
            value: self.cs.encode(value)?,
        });
        Ok(())
    }
    pub fn finish(self) -> Record {
        Record {
            fields: self.fields,
        }
    }
}
