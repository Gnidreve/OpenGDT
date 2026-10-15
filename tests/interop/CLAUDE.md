# CLAUDE.md – tests/interop/

* Reines Dev-Werkzeug. Die Abhängigkeiten (`pynetdicom`, `pillow`, `numpy`) dürfen **nie**
  in die Rust-Crates oder deren Build wandern (Projektregel: std-only).
* `run.sh` räumt sich selbst auf (Wegwerf-Verzeichnis, Bridge wird beendet).
* Port 11112 ist fest verdrahtet; läuft dort schon etwas, schlägt der Test fehl.
* Bei Änderungen an Worklist-Attributen, Accession-Format oder GDT-Ausgabe hier gegenprüfen.
* Hinweis für Agenten: Bridge **nicht** mit `pkill -f opengdt-bridge` beenden, das trifft
  auch die eigene Shell-Kommandozeile. `pkill -x opengdt-bridge` verwenden.
