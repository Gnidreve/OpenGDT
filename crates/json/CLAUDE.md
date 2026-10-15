# CLAUDE.md – crates/json

* Handgeschriebener Parser (Projektregel: std-only). Jede Änderung braucht Tests für gültige **und** kaputte Eingaben;
  er darf bei beliebigem Input nie panicen (Präfix-Test in `src/lib.rs`). Eingaben kommen auch aus dem Netzwerk
  (Orthanc), nicht nur aus Dateien – Limits (Tiefe 64) nicht aufweichen.
* Keine Serialisierung ergänzen, solange niemand sie braucht.
