# opengdt-bridge

Die ausführbare Bridge. Verbindet die Crates zu einem Dienst.

```sh
cargo build --release -p opengdt-bridge
./target/release/opengdt-bridge run opengdt.conf        # Dienst starten
./target/release/opengdt-bridge check-config opengdt.conf
./target/release/opengdt-bridge echo 192.168.1.20:104 GERAET_AE   # DICOM-Verbindungstest
```

Eine kommentierte Beispielkonfiguration liegt in [`opengdt.conf.example`](opengdt.conf.example).
Alle Schlüssel sind optional, unbekannte Schlüssel sind ein Fehler.

## Ablauf

1. **Watcher** (`watcher.rs`) prüft jede Sekunde `gdt_import_dir` auf `*.gdt`. Eine Datei gilt als
   fertig, wenn Größe und Änderungszeit zwei Durchläufe lang gleich blieben. Sie wird geparst,
   als Worklist-Eintrag abgelegt und danach gelöscht (oder nach `processed/` verschoben).
   Fehlerhafte Dateien werden zu `<name>.gdt.err` – so läuft nichts in einer Endlosschleife.
2. **Handler** (`handler.rs`) beantwortet Worklist-Abfragen und archiviert eingehende Bilder.
3. **Pipeline** (`pipeline.rs`) sammelt Bilder je Untersuchung (StudyInstanceUID). Nach
   `settle_seconds` Ruhe werden sie konvertiert, nach `export_dir/<PatientID>/<Datum>_<Accession>/`
   geschrieben und **eine** GDT-Rückmeldung nach `gdt_export_dir` erzeugt. Sendet das Gerät
   jedes Bild in einer eigenen Association, bleibt es trotzdem bei einer Rückmeldung.
4. Ist `gdt_result_filename` gesetzt und die Datei noch da (PVS hat sie nicht abgeholt),
   wird **nicht überschrieben**; die Untersuchung wartet und wird nachgeliefert.

## Betrieb

* Logs gehen auf stderr (`Zeitstempel LEVEL Nachricht`), der Prozess läuft im Vordergrund.
  Als Dienst (systemd, Windows-Dienst/NSSM) starten. Ein Signal-Handler (sauberes Beenden)
  existiert nicht (`std` bietet keinen). Das ist unkritisch: Originale werden sofort archiviert,
  und beim nächsten Start werden noch nicht zurückgemeldete Untersuchungen automatisch
  nachgeliefert (Marker `.reported` im Archiv).
* Port 11112 (Standard) in der Firewall freigeben; DICOM ist hier unverschlüsselt.
