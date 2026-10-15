# tests/orthanc

Ende-zu-Ende gegen ein **echtes Orthanc**: `pynetdicom` (als Ultraschallgerät) → Orthanc → Bridge im Orthanc-Modus →
GDT-Rückmeldung und Bilddateien. Zusätzlich läuft die komplette Worklist-Abfrage des DP-50 (aus dem
[Geräteprofil](../../devices/mindray-dp/README.md)) gegen das Worklist-Plugin von Orthanc.

```sh
tests/orthanc/run.sh
```

Voraussetzungen: `cargo`, Python 3 mit `venv` und Internetzugang (nur für diesen Test), ein installiertes **Orthanc**
(`apt install orthanc`; fehlt es, wird der Test übersprungen). Orthanc startet mit
[`docker/orthanc/orthanc.json`](../../docker/orthanc/orthanc.json), also der ausgelieferten Konfiguration (nur Pfade und Ports ersetzt).

Den Docker-Stack selbst prüft [`docker/smoke.sh`](../../docker/smoke.sh).
