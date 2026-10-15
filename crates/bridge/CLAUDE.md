# CLAUDE.md – crates/bridge

## Aufbau
`lib.rs` (`start(Config) -> Running`: Server-, Watcher-, Flusher-Thread) · `config.rs` ·
`watcher.rs` · `handler.rs` (`BridgeHandler` implementiert `dicom::scp::Handler`) ·
`pipeline.rs` · `log.rs` (Makros `info!/warn!/error!`) · `main.rs` (CLI).

## Invarianten und Fallen
* **Original zuerst:** `on_store` archiviert und antwortet `Success`, *bevor* konvertiert wird.
  Konvertierungsfehler dürfen nie zu einem DICOM-Fehlerstatus führen (Gerät würde endlos wiederholen).
* `Pipeline.pending` lebt im Speicher, ist aber wiederherstellbar: Nach erfolgreicher Rückmeldung
  liegt im Archivverzeichnis der Untersuchung ein Marker `.reported`; `Pipeline::recover` nimmt beim Start
  alle Archiv-Untersuchungen ohne Marker wieder auf. Nachzügler-Bilder zu bereits gemeldeten
  Untersuchungen erzeugen eine zweite, kleinere Rückmeldung (nur die neuen Bilder).
* GDT-Eingangsdatei muss nach Verarbeitung weg oder umbenannt sein, sonst Endlosschleife
  (`finish_ok` loggt dann ERROR).
* `gdt_import_dir != gdt_export_dir` wird in der Konfiguration erzwungen.
* Zeitzone: nur `utc_offset_minutes` (kein Sommerzeit-Handling) – siehe `worklist/README.md`.
* Tests: `tests/e2e.rs` startet die komplette Bridge auf Port 0 mit Wegwerf-Verzeichnissen.
  Neue Konfigurationsschlüssel → `config.rs` (Parser + Default + Test), `opengdt.conf.example`,
  diese README.
* Nie `pkill -f opengdt-bridge` in Shell-Befehlen mit diesem Text verwenden (trifft die eigene Shell);
  `pkill -x opengdt-bridge`.
