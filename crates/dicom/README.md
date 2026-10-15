# opengdt-dicom

Minimaler DICOM-Stack ohne Abhängigkeiten – gerade so viel, wie die Bridge braucht.

| Modul | Inhalt |
|---|---|
| `dataset` | `Dataset`/`Element`/`Value`: Aufbau, Encoding, Decoding (Implicit & Explicit VR Little Endian, Sequenzen, Encapsulated Pixel Data) |
| `tag` | Tags (`t::PATIENT_NAME` …), VR-Typen, kleines Datenwörterbuch |
| `file` | DICOM Part 10 Dateien lesen/schreiben |
| `pdu` | Upper Layer PDUs (A-ASSOCIATE-RQ/AC/RJ, P-DATA-TF, RELEASE, ABORT) |
| `assoc` | `Association`: DIMSE-Nachrichten senden/empfangen, Fragmentierung |
| `scp` | `Server` + `Handler`-Trait: beantwortet **C-ECHO**, **C-FIND** (Worklist), **C-STORE** |
| `scu` | Mini-Client für Selbsttests und `bridge echo` |
| `uid` | UID-Konstanten, UID-Erzeugung (`2.25.…`) |

## Server benutzen

```rust
struct MyHandler;
impl Handler for MyHandler {
    fn supports_store(&self) -> bool { true }
    fn on_store(&self, peer: &PeerInfo, obj: StoredObject) -> u16 { /* … */ 0x0000 }
}
let server = Server::bind("0.0.0.0:11112", ServerConfig::default(), Arc::new(MyHandler))?;
server.run(); // blockiert; stop_flag() zum Beenden
```

* Ein Thread je Verbindung, Lese-/Schreib-Timeout 60 s, höchstens 32 gleichzeitige Verbindungen.
* Transfer Syntax bei Storage: die **erste** vorgeschlagene, die nicht Big Endian/Deflate ist
  (die Reihenfolge der Gegenseite wird respektiert). Bei Worklist: Explicit, sonst Implicit VR LE.
* `strict_called_ae = false` (Standard): Called-AE wird nicht geprüft.

## Grenzen

* Nur Little Endian; kein Deflate; keine Character Sets außer ASCII/Latin-1/UTF-8 beim Lesen.
* Ein C-STORE-Datensatz wird komplett im Speicher gepuffert (große Cine-Clips = große Puffer).
* Kein TLS, keine Nutzerauthentifizierung, kein C-MOVE/C-GET/Query-Retrieve, kein N-CREATE/N-SET (MPPS).
* Das Wörterbuch enthält nur die Tags, die das Projekt benutzt; unbekannte Tags sind `UN`.
