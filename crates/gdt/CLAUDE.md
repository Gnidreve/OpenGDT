# CLAUDE.md – crates/gdt

## Aufbau
* `charset.rs` – `Charset` (Feld 9206), CP437-Tabelle eingebettet (aus Python `cp437` erzeugt).
* `record.rs` – `Record`/`Field`: Parsen (strict/lenient) und Serialisieren inkl. 8100.
* `patient.rs` – `Patient`, `ExamResult`, `ImageRef`, `WriteOptions`, Datumskonvertierung
  `TTMMJJJJ` ⇄ `YYYYMMDD`.
* `lib.rs` – Feldkennungen in `ids`, `Error`, Tests.

## Invarianten
* Zeilenlänge zählt **Bytes**, nicht Zeichen (Latin-1/CP437 sind 1 Byte/Zeichen, UTF-8 nicht).
* Feld 8100 steht direkt hinter 8000 und enthält die Gesamtlänge der Datei in Bytes.
* `Record::to_bytes` ignoriert ein evtl. vorhandenes 8100 im Eingabesatz und berechnet neu.
* Nicht darstellbare Zeichen → `Error::Encoding` (kein stilles Ersetzen). Die Bridge ersetzt
  vorher selbst mit `?` (`bridge/src/pipeline.rs::lossy`).

## Ungeprüft (nicht gegen die offizielle QMS-Spezifikation verifiziert)
Diese Werte stammen aus dem Gedächtnis und **müssen** gegen die Spezifikation bzw. echte
Dateien des Ziel-PVS geprüft werden, bevor das Projekt produktiv geht:
* Satzart der Anforderung (`6301`) und der Rückmeldung (`6310`, konfigurierbar `gdt_result_type`).
* Dateiverweis-Felder `6303` (Format), `6304` (Beschreibung), `6305` (Pfad) in `ids`.
* Versionsfeld 9218 = `02.10`.
* Feld 6228 (Anmerkung) ist definiert, wird aber noch nicht geschrieben.
Sicher (Standardwissen): Zeilenformat, 8000, 8100, 8315, 8316, 9206, 3000, 3101–3103, 3110, 6200, 6201.
