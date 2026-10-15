# CLAUDE.md – docker/

* Gilt zusammen mit `Dockerfile`, `docker-compose.yml`, `.env.example` im Repository-Wurzelordner.
* **Alles hier wurde gegen reale Images getestet** (Docker-Daemon läuft in der Sandbox mit `--storage-driver=vfs`; Images über
  `mirror.gcr.io`, weil Docker Hub rate-limitiert: `RUST_IMAGE=mirror.gcr.io/library/rust:1-slim-bookworm`,
  `ORTHANC_IMAGE=mirror.gcr.io/jodogne/orthanc-plugins:latest`). Der Plugin-Pfad im Orthanc-Image unterscheidet sich vom
  Debian-Paket – nie aus dem Gedächtnis ändern, sondern im Image nachsehen (`docker run --rm --entrypoint sh … find / -name '*Worklist*'`).
* `orthanc.json` wird auch von `tests/orthanc/run.sh` verwendet (nur Pfade ersetzt). Änderungen dort immer mit
  `tests/orthanc/run.sh` und `docker/smoke.sh` gegenprüfen.
* **Orthanc-Worklist verlangt exakt passenden Station-AE** (leer/fehlend matcht nicht, empirisch geprüft). Deshalb Worklist am
  Gerät → Bridge (11112); nicht auf „einfachere“ Alleinlösung über Orthanc umbauen, ohne diese Eigenschaft zu bedenken.
* Konfiguration der Bridge: `docker/bridge.conf` + Umgebung `OPENGDT_<SCHLÜSSEL>` (siehe `docker-compose.yml`). Neue Konfigurationsschlüssel,
  die der Stack braucht, dort ergänzen und in `.env.example` dokumentieren.
* Dateirechte: Der `init`-Dienst legt `./data` mit `PUID:PGID` an; die Bridge läuft nicht als root. Nie `user:` entfernen, sonst
  gehören die Dateien root und das PVS kann sie nicht löschen.
* Nicht ändern ohne Rückfrage: Veröffentlichte Ports, Orthanc-Authentifizierung (aktuell aus, UI nur localhost), Image-Quellen.
* Nach Tests in der Sandbox aufräumen: `docker compose down -v; rm -rf data`.
