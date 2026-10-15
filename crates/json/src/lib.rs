//! Minimaler JSON-Parser (RFC 8259) auf `std`-Basis. Reicht für Profildateien und die
//! Orthanc-REST-Antworten; kein Serialisieren, keine Streaming-API.

#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    /// Reihenfolge der Schlüssel bleibt erhalten.
    Obj(Vec<(String, Json)>),
}

impl Json {
    pub fn parse(text: &str) -> Result<Json, String> {
        let mut p = Parser {
            s: text.as_bytes(),
            pos: 0,
        };
        p.ws();
        let v = p.value(0)?;
        p.ws();
        if p.pos != p.s.len() {
            return Err(p.err("unerwartete Zeichen nach dem Wert"));
        }
        Ok(v)
    }

    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(o) => o.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// Pfad mit Punkten, z. B. `"meta.source.url"`.
    pub fn at(&self, path: &str) -> Option<&Json> {
        path.split('.').try_fold(self, |j, k| j.get(k))
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Json::Num(n) if *n >= 0.0 && n.fract() == 0.0 => Some(*n as u64),
            _ => None,
        }
    }

    pub fn as_arr(&self) -> Option<&[Json]> {
        match self {
            Json::Arr(a) => Some(a),
            _ => None,
        }
    }

    pub fn as_obj(&self) -> Option<&[(String, Json)]> {
        match self {
            Json::Obj(o) => Some(o),
            _ => None,
        }
    }
}

const MAX_DEPTH: usize = 64;

struct Parser<'a> {
    s: &'a [u8],
    pos: usize,
}

impl Parser<'_> {
    fn err(&self, msg: &str) -> String {
        let before = &self.s[..self.pos.min(self.s.len())];
        let line = before.iter().filter(|&&b| b == b'\n').count() + 1;
        let col = before.len()
            - before
                .iter()
                .rposition(|&b| b == b'\n')
                .map_or(0, |i| i + 1)
            + 1;
        format!("JSON Zeile {line}, Spalte {col}: {msg}")
    }

    fn ws(&mut self) {
        while matches!(self.s.get(self.pos), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.pos += 1;
        }
    }

    fn eat(&mut self, lit: &str) -> bool {
        if self.s[self.pos..].starts_with(lit.as_bytes()) {
            self.pos += lit.len();
            true
        } else {
            false
        }
    }

    fn value(&mut self, depth: usize) -> Result<Json, String> {
        if depth > MAX_DEPTH {
            return Err(self.err("zu tief verschachtelt"));
        }
        match self.s.get(self.pos) {
            None => Err(self.err("unerwartetes Ende")),
            Some(b'{') => self.object(depth),
            Some(b'[') => self.array(depth),
            Some(b'"') => Ok(Json::Str(self.string()?)),
            Some(b't') if self.eat("true") => Ok(Json::Bool(true)),
            Some(b'f') if self.eat("false") => Ok(Json::Bool(false)),
            Some(b'n') if self.eat("null") => Ok(Json::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(_) => Err(self.err("unerwartetes Zeichen")),
        }
    }

    fn object(&mut self, depth: usize) -> Result<Json, String> {
        self.pos += 1;
        let mut out = Vec::new();
        self.ws();
        if self.s.get(self.pos) == Some(&b'}') {
            self.pos += 1;
            return Ok(Json::Obj(out));
        }
        loop {
            self.ws();
            if self.s.get(self.pos) != Some(&b'"') {
                return Err(self.err("Schlüssel (String) erwartet"));
            }
            let k = self.string()?;
            if out.iter().any(|(e, _): &(String, Json)| *e == k) {
                return Err(self.err(&format!("doppelter Schlüssel '{k}'")));
            }
            self.ws();
            if self.s.get(self.pos) != Some(&b':') {
                return Err(self.err("':' erwartet"));
            }
            self.pos += 1;
            self.ws();
            out.push((k, self.value(depth + 1)?));
            self.ws();
            match self.s.get(self.pos) {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Json::Obj(out));
                }
                _ => return Err(self.err("',' oder '}' erwartet")),
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<Json, String> {
        self.pos += 1;
        let mut out = Vec::new();
        self.ws();
        if self.s.get(self.pos) == Some(&b']') {
            self.pos += 1;
            return Ok(Json::Arr(out));
        }
        loop {
            self.ws();
            out.push(self.value(depth + 1)?);
            self.ws();
            match self.s.get(self.pos) {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Json::Arr(out));
                }
                _ => return Err(self.err("',' oder ']' erwartet")),
            }
        }
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let h = self
            .s
            .get(self.pos..self.pos + 4)
            .ok_or_else(|| self.err("\\u abgeschnitten"))?;
        let v = std::str::from_utf8(h)
            .ok()
            .and_then(|h| u32::from_str_radix(h, 16).ok())
            .ok_or_else(|| self.err("ungültiges \\u-Escape"))?;
        self.pos += 4;
        Ok(v)
    }

    fn string(&mut self) -> Result<String, String> {
        self.pos += 1; // öffnendes "
        let mut out = String::new();
        loop {
            let start = self.pos;
            while let Some(&b) = self.s.get(self.pos) {
                if b == b'"' || b == b'\\' || b < 0x20 {
                    break;
                }
                self.pos += 1;
            }
            out.push_str(
                std::str::from_utf8(&self.s[start..self.pos])
                    .map_err(|_| self.err("ungültiges UTF-8"))?,
            );
            match self.s.get(self.pos) {
                Some(b'"') => {
                    self.pos += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.pos += 1;
                    let e = *self
                        .s
                        .get(self.pos)
                        .ok_or_else(|| self.err("Escape abgeschnitten"))?;
                    self.pos += 1;
                    match e {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let hi = self.hex4()?;
                            let cp = if (0xD800..0xDC00).contains(&hi) {
                                if !self.eat("\\u") {
                                    return Err(self.err("Surrogat-Paar unvollständig"));
                                }
                                let lo = self.hex4()?;
                                if !(0xDC00..0xE000).contains(&lo) {
                                    return Err(self.err("ungültiges Surrogat-Paar"));
                                }
                                0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00)
                            } else {
                                hi
                            };
                            out.push(
                                char::from_u32(cp)
                                    .ok_or_else(|| self.err("ungültiger Codepoint"))?,
                            );
                        }
                        _ => return Err(self.err("unbekanntes Escape")),
                    }
                }
                Some(_) => return Err(self.err("Steuerzeichen in String")),
                None => return Err(self.err("String nicht beendet")),
            }
        }
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.pos;
        if self.s[self.pos] == b'-' {
            self.pos += 1;
        }
        let digits = |p: &mut Self| {
            let s = p.pos;
            while matches!(p.s.get(p.pos), Some(b'0'..=b'9')) {
                p.pos += 1;
            }
            p.pos > s
        };
        if self.s.get(self.pos) == Some(&b'0') {
            self.pos += 1;
        } else if !digits(self) {
            return Err(self.err("Ziffer erwartet"));
        }
        if self.s.get(self.pos) == Some(&b'.') {
            self.pos += 1;
            if !digits(self) {
                return Err(self.err("Nachkommastellen erwartet"));
            }
        }
        if matches!(self.s.get(self.pos), Some(b'e' | b'E')) {
            self.pos += 1;
            if matches!(self.s.get(self.pos), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            if !digits(self) {
                return Err(self.err("Exponent erwartet"));
            }
        }
        let t = std::str::from_utf8(&self.s[start..self.pos]).unwrap_or("");
        t.parse()
            .map(Json::Num)
            .map_err(|_| self.err("ungültige Zahl"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_types() {
        let j =
            Json::parse(r#" {"a": [1, -2.5, 1e3, true, false, null], "b": {"c": "x"}, "d": []} "#)
                .unwrap();
        assert_eq!(j.at("b.c").and_then(Json::as_str), Some("x"));
        let a = j.get("a").unwrap().as_arr().unwrap();
        assert_eq!(a[0].as_u64(), Some(1));
        assert_eq!(a[1], Json::Num(-2.5));
        assert_eq!(a[2], Json::Num(1000.0));
        assert_eq!(a[3].as_bool(), Some(true));
        assert_eq!(a[5], Json::Null);
        assert_eq!(j.get("d").unwrap().as_arr().unwrap().len(), 0);
        assert_eq!(a[1].as_u64(), None);
    }

    #[test]
    fn string_escapes_and_unicode() {
        let j = Json::parse(r#""a\n\t\"\\\/ \u00fc \ud83d\ude00 ä""#).unwrap();
        assert_eq!(j.as_str(), Some("a\n\t\"\\/ ü 😀 ä"));
    }

    #[test]
    fn keeps_key_order_and_rejects_duplicates() {
        let j = Json::parse(r#"{"z":1,"a":2}"#).unwrap();
        assert_eq!(j.as_obj().unwrap()[0].0, "z");
        assert!(Json::parse(r#"{"a":1,"a":2}"#)
            .unwrap_err()
            .contains("doppelter"));
    }

    #[test]
    fn errors_have_positions_and_never_panic() {
        let e = Json::parse("{\n  \"a\": ,\n}").unwrap_err();
        assert!(e.contains("Zeile 2"), "{e}");
        for bad in [
            "",
            "{",
            "[1,",
            "\"abc",
            "tru",
            "{\"a\"}",
            "[1 2]",
            "01",
            "1.",
            "-",
            "\"\\u12\"",
            "\"\\ud800\"",
            "{}x",
            "\u{1}",
        ] {
            assert!(Json::parse(bad).is_err(), "{bad:?}");
        }
        // Mutationstest: jedes Präfix eines gültigen Dokuments darf nur Err oder Ok liefern, nie panicen
        let doc = r#"{"a":[1,{"b":"c\u00e4"}],"d":true}"#;
        for i in 0..doc.len() {
            if doc.is_char_boundary(i) {
                let _ = Json::parse(&doc[..i]);
            }
        }
    }

    #[test]
    fn depth_limit() {
        let deep = "[".repeat(200) + &"]".repeat(200);
        assert!(Json::parse(&deep).unwrap_err().contains("tief"));
    }
}
