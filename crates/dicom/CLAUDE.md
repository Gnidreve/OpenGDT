# CLAUDE.md – crates/dicom

## Wichtige Invarianten
* **Command-Datensätze sind immer Implicit VR LE**, unabhängig vom Presentation Context
  (`assoc.rs`). Nutzdaten folgen der verhandelten Transfer Syntax.
* `Dataset` ist nach Tag sortiert (`BTreeMap`), das Encoding ist deterministisch.
* Ungerade Werte werden beim Encoding aufgefüllt (`UI` mit `0x00`, sonst Leerzeichen/`0x00`
  je nach VR); `get_str` entfernt `\0` und Leerzeichen am Ende.
* Das Decoding darf bei kaputten Eingaben **nie panicen** (Test `truncated_input_is_error_not_panic`).
  Längen immer gegen die vorhandenen Bytes prüfen. PDU-Größen sind auf 16 MiB begrenzt.
* **Limits sind Sicherheitsmerkmale** (alle nachweislich nötig, siehe Tests): `MAX_NESTING` (32) im Dataset-Decoder – ohne ihn
  bringt ein präparierter Datensatz den **ganzen Prozess** per Stack Overflow zum Absturz (`tests/nesting.rs`); `MAX_PDU` (1 MiB);
  `max_message_bytes` (512 MiB). Nicht erhöhen/entfernen ohne Grund. Neue Decoder für Netzwerkdaten brauchen ein Tiefen-/Größenlimit
  und einen Fall in `tests/fuzz_smoke.rs`.
* Der `Server` darf durch fehlerhafte Clients nicht sterben (Test `garbage_does_not_kill_server`).
* Anzahl SH-Werte ≤ 16 Zeichen, AE ≤ 16, UID ≤ 64 – das prüft der Stack **nicht**, Aufrufer müssen es.

* Implicit VR: Sequenz-Tags außerhalb des Wörterbuchs werden über den Item-Marker (`FFFE,E000`) am
  Wertanfang als Sequenz erkannt (`decode_elements`, Test `implicit_vr_unknown_tag_with_items_…`).
  Ohne diese Heuristik werden Return-Key-Sequenzen von Geräten zu Rohbytes.
* Storage-Transfer-Syntax: `ServerConfig::storage_transfer_syntaxes` (Präferenz) vor der Reihenfolge
  der Gegenseite; nie Big Endian/Deflate.

## Tests
* `tests/loopback.rs` – Rust-SCU gegen Rust-SCP (beweist nur Selbstkonsistenz!).
* `tests/protocol.rs` – Fehler- und Randfälle: abgelehnte Contexts (Grund 4), Protokollverletzungen, überlanges PDU,
  Verbindungslimit, Read-Timeout, Fake-SCP (`fake_scp`) für Fehlverhalten der Gegenstelle gegenüber dem SCU.
  Neue Fehlerpfade hier mit einer Gegenstelle testen, die sich falsch verhält – nicht nur mit dem eigenen SCU.
* Echte Interoperabilität: `tests/interop/run.sh` (pynetdicom). Bei Protokolländerungen
  immer laufen lassen.

## Erweiterungsideen (noch nicht umgesetzt)
Streaming großer Datensätze auf Platte, TLS, MPPS (N-CREATE) für „Untersuchung abgeschlossen“,
weitere Transfer Syntaxes/Character Sets.
