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
* `orthanc.rs`: Poller gegen Orthanc-REST. Fehler tragen `permanent` (404, unzulässige ID → sofort überspringen) oder nicht (Wiederholen,
  nach `MAX_ATTEMPTS` überspringen) – damit eine kaputte Änderung nie die Warteschlange blockiert. Orthanc-IDs vor dem Einsetzen in
  URL-Pfade prüfen (`safe_id`). Tests: `tests/orthanc_mode.rs` (Fake-Orthanc, Fehlerfälle); der Normalfall gegen echtes Orthanc in
  `tests/orthanc/run.sh`. Die API-Formen sind gegen Orthanc 1.12 geprüft – bei neuen Endpunkten dort gegenprüfen.
* `http.rs`: eigener HTTP/1.0-Client (GET, nur `http://`), Antwortlimit 512 MiB, Chunked-Dekodierung vorhanden.
* Umgebungsüberschreibung `OPENGDT_*` wird in `main.rs` als zusätzliche Konfigurationszeilen angehängt (gleicher Parser, gleiche Validierung).
* Nie `pkill -f opengdt-bridge` in Shell-Befehlen mit diesem Text verwenden (trifft die eigene Shell);
  `pkill -x opengdt-bridge`.
