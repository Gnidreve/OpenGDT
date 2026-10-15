# CLAUDE.md – crates/convert

* `png.rs` ist unabhängig validiert (Pillow liest die Ausgabe, Testdaten: Graustufen und RGB).
  Bei Änderungen die Ausgabe erneut extern prüfen:
  `OPENGDT_DUMP=<verzeichnis> cargo test -p opengdt-convert` schreibt `gray.png`/`rgb.png`.
* JPEG-Padding: Fragmente sind auf gerade Länge aufgefüllt; `strip_pad` entfernt nur Nullbytes
  *hinter* einem fehlenden EOI-Marker. Echte JPEGs enden auf `FF D9`.
* Frame-Zuordnung (`split_frames`): 1 Frame → alle Fragmente; sonst ein Fragment je Frame;
  sonst Basic Offset Table. Ist nichts davon eindeutig → `Unsupported` (kein Raten).
* `avi.rs`: eigener RIFF-Writer. Validiert gegen ffmpeg (PyAV) in `tests/interop/interop.py` – bei Änderungen
  immer `tests/interop/run.sh` laufen lassen. Fallen, die schon aufgetreten sind: DIB-Zeilen sind **bottom-up**
  und auf 4 Byte gepolstert (ffmpeg meldet dann negativen `line_size`); `idx1`-Offsets zählen ab dem
  `movi`-Kennwort; Chunks sind auf gerade Länge gepolstert, die Chunk-Länge enthält das Füllbyte **nicht**.
* MJPG-Frames werden nicht dekodiert; Größe aus dem SOF-Marker (`jpeg_dimensions`), Ersatz Rows/Columns.
  Frames unterschiedlicher Größe → `Invalid`. DICOM-Füllbytes hinter dem EOI werden vorher entfernt.
* `Options::multi_frame` ersetzt das frühere `all_frames`; Einzelbild-Tests nutzen `STILLS` (`FirstFrame`).
* Kein Panic bei kaputten Eingaben: Größen gegen vorhandene Bytes prüfen.
* Neue Formate: Test mit synthetischem `Dataset` in `src/tests.rs`, Doku-Tabelle in README anpassen.
