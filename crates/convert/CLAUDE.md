# CLAUDE.md – crates/convert

* `png.rs` ist unabhängig validiert (Pillow liest die Ausgabe, Testdaten: Graustufen und RGB).
  Bei Änderungen die Ausgabe erneut extern prüfen:
  `OPENGDT_DUMP=<verzeichnis> cargo test -p opengdt-convert` schreibt `gray.png`/`rgb.png`.
* JPEG-Padding: Fragmente sind auf gerade Länge aufgefüllt; `strip_pad` entfernt nur Nullbytes
  *hinter* einem fehlenden EOI-Marker. Echte JPEGs enden auf `FF D9`.
* Frame-Zuordnung (`split_frames`): 1 Frame → alle Fragmente; sonst ein Fragment je Frame;
  sonst Basic Offset Table. Ist nichts davon eindeutig → `Unsupported` (kein Raten).
* Kein Panic bei kaputten Eingaben: Größen gegen vorhandene Bytes prüfen.
* Neue Formate: Test mit synthetischem `Dataset` in `src/tests.rs`, Doku-Tabelle in README anpassen.
