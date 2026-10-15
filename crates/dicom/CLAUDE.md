# CLAUDE.md – crates/dicom

## Wichtige Invarianten
* **Command-Datensätze sind immer Implicit VR LE**, unabhängig vom Presentation Context
  (`assoc.rs`). Nutzdaten folgen der verhandelten Transfer Syntax.
* `Dataset` ist nach Tag sortiert (`BTreeMap`), das Encoding ist deterministisch.
* Ungerade Werte werden beim Encoding aufgefüllt (`UI` mit `0x00`, sonst Leerzeichen/`0x00`
  je nach VR); `get_str` entfernt `\0` und Leerzeichen am Ende.
* Das Decoding darf bei kaputten Eingaben **nie panicen** (Test `truncated_input_is_error_not_panic`).
  Längen immer gegen die vorhandenen Bytes prüfen. PDU-Größen sind auf 16 MiB begrenzt.
* Der `Server` darf durch fehlerhafte Clients nicht sterben (Test `garbage_does_not_kill_server`).
* Anzahl SH-Werte ≤ 16 Zeichen, AE ≤ 16, UID ≤ 64 – das prüft der Stack **nicht**, Aufrufer müssen es.

## Tests
* `tests/loopback.rs` – Rust-SCU gegen Rust-SCP (beweist nur Selbstkonsistenz!).
* Echte Interoperabilität: `tests/interop/run.sh` (pynetdicom). Bei Protokolländerungen
  immer laufen lassen.

## Erweiterungsideen (noch nicht umgesetzt)
Streaming großer Datensätze auf Platte, TLS, MPPS (N-CREATE) für „Untersuchung abgeschlossen“,
weitere Transfer Syntaxes/Character Sets.
