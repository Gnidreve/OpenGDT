# CLAUDE.md – devices/

* Je Gerätefamilie ein Ordner mit `README.md`, `CLAUDE.md` und dem Profil (`*.json`, Schema
  `opengdt-device-profile/1`, siehe `crates/profile/README.md`).
* Profile sind **Abschriften aus Herstellerdokumenten**, keine Messungen. Pflicht in `meta`: Quelle (URL,
  Titel, Teilenummer, Version, SHA-256 der Datei) und `capture` (Zeitpunkt in UTC, wer/was abgeschrieben hat,
  `review_status`). Nie Werte „ergänzen“, die nicht im Dokument stehen.
* **Die Tests leiten ihre Erwartungen aus dem Profil ab** (`crates/bridge/tests/device_profiles.rs`). Ein neues Profil
  = neuer Teststand, ohne Testcode zu ändern. Neue Testfälle sollen weiter aus dem Profil lesen, nichts hartkodieren,
  was im Profil steht.
* Aktualisierung: siehe `capture.update_procedure` im Profil. Danach `cargo test --workspace`; schlägt etwas fehl,
  ist entweder die Abschrift fehlerhaft oder der Code passt nicht mehr zum Gerät – beides sind echte Befunde.
* Die Bridge liest Profile **nicht zur Laufzeit** (gerätegenerisch, kein JSON im Produktivcode).
* Neues Gerät: Ordner anlegen, Tabelle in `devices/README.md` ergänzen, Testdatei ist nicht nötig: `device_profiles.rs` prüft jedes `devices/*/dicom-conformance.json`.
