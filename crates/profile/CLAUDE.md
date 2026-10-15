# CLAUDE.md – crates/profile

* Der JSON-Parser ist handgeschrieben (Projektregel: std-only). Jede Änderung braucht Tests für gültige
  **und** kaputte Eingaben; er darf bei beliebigem Input nie panicen (Präfix-Test in `json.rs`).
* Validierung ist absichtlich streng (Tags nur Großbuchstaben-Hex, UIDs ohne führende Nullen, Zeitstempel
  strikt `…Z`), damit Neu-Abschriften diffbar und maschinell prüfbar bleiben.
* Neue Pflichtfelder im Schema: `validate()` + Negativtest in `profile.rs` (Muster `expect_error`) + Schema-Version
  erhöhen, falls bestehende Profile ungültig würden. Änderungen am Schema in README und `devices/CLAUDE.md` nachziehen.
* Die Tests lesen das echte Profil unter `devices/mindray-dp/` (Pfad über `CARGO_MANIFEST_DIR`). Wer die Datei
  verschiebt, muss die Pfade in `src/profile.rs` (Tests) und `crates/bridge/tests/mindray_dp.rs` anpassen.
* Nicht zur Laufzeit der Bridge einbinden, solange es keine Entscheidung dafür gibt (die Bridge ist gerätegenerisch).
