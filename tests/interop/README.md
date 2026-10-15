# tests/interop

Optionaler Ende-zu-Ende-Test gegen eine **unabhängige** DICOM-Implementierung
([`pynetdicom`](https://pydicom.github.io/pynetdicom/)). Er beweist, dass die selbst
geschriebene DICOM-Schicht nicht nur mit sich selbst redet.

```sh
tests/interop/run.sh
```

Das Skript baut die Bridge, startet sie in einem Wegwerf-Verzeichnis, legt eine
GDT-Anforderung ab und lässt `pynetdicom` dann Verbindungstest (C-ECHO), Worklist-Abfrage
(C-FIND) und Bildversand (C-STORE; JPEG Baseline und unkomprimiert) ausführen. Danach
werden die exportierten Dateien mit Pillow geöffnet und die GDT-Rückmeldung angezeigt.

**Abhängigkeiten gelten nur für diesen Test**, nie für das Projekt selbst: Python 3, `pip`
(installiert `pynetdicom`, `pillow`, `numpy`, `av` in ein temporäres venv) und Internetzugang.
Die normalen Tests (`cargo test --workspace`) brauchen nichts davon.
