# opengdt-gdt

Liest und schreibt **GDT**-Dateien (Gerätedatentransfer). Kein I/O, keine Abhängigkeiten.

## Format

Jede Zeile: `LLL` (3 Ziffern, Zeilenlänge inkl. `CR LF`) + `FFFF` (4 Ziffern, Feldkennung) +
Inhalt + `CR LF`. Beispiel: `0138000` + `6301` + `CRLF` = Satzart 6301.

## API

```rust
use opengdt_gdt::{Record, Patient, ExamResult, ImageRef, WriteOptions, Charset};

// Lesen
let rec = Record::parse(&bytes, /* strict */ false)?;
let patient = Patient::from_record(&rec)?;   // Nr., Name, Geburtsdatum (→ YYYYMMDD), Geschlecht

// Schreiben
let bytes = ExamResult { patient, exam_date, exam_time, images }
    .to_record(&WriteOptions { record_type: "6310", receiver_id: "PVS",
                               sender_id: "OPENGDT", charset: Charset::Latin1 })?
    .to_bytes();                              // Feld 8100 (Satzlänge) wird berechnet
```

* **Zeichensätze** (Feld 9206): 7-Bit, IBM-PC (CP437), ISO 8859-1; zusätzlich `Utf8` als
  Erweiterung, nur per Konfiguration wählbar.
* **Lenient parsen** (Standard): Zeilen werden an Zeilenenden getrennt, falsche Längen und
  reine `LF`-Dateien werden toleriert. **Strict** prüft jede Zeilenlänge.

## Grenzen

Nur die Felder, die die Bridge braucht (Patientenstammdaten, Untersuchungsdatum, Dateiverweise).
Kein vollständiges GDT-Objektmodell. Einige Feldkennungen sind noch **ungeprüft**
(siehe `CLAUDE.md`).
