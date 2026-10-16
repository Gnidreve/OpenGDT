# CLAUDE.md – docker/

* Gilt zusammen mit `Dockerfile`, `docker-compose.yml`, `.env.example` im Repository-Wurzelordner.
* **Alles hier wurde gegen reale Images getestet** (Docker-Daemon läuft in der Sandbox mit `--storage-driver=overlay2`, `vfs` füllt die Platte; Images über
  `mirror.gcr.io`, weil Docker Hub rate-limitiert: `RUST_IMAGE=mirror.gcr.io/library/rust:1-slim-bookworm`,
  `ORTHANC_IMAGE=mirror.gcr.io/orthancteam/orthanc:latest`). Das Image `orthancteam/orthanc` (amd64+arm64) baut seine Konfiguration selbst: liest `/etc/orthanc/*.json`, aktiviert Plugins
  per Umgebungsvariable (`WORKLISTS_PLUGIN_ENABLED`) bzw. wenn ihr Abschnitt (`Worklists`) in der Konfiguration steht, und trägt
  den Plugin-Pfad selbst ein. In `orthanc.json` deshalb **kein** `Plugins`-Eintrag (er würde den Standard überschreiben). Bei Zweifeln
  im Image nachsehen (`/startup/*.py`, `/startup/plugins-def.json`), nicht aus dem Gedächtnis ändern.
* `orthanc.json` wird auch von `tests/orthanc/run.sh` verwendet (nur Pfade ersetzt). Änderungen dort immer mit
  `tests/orthanc/run.sh` und `docker/smoke.sh` gegenprüfen.
* **Orthanc-Worklist verlangt exakt passenden Station-AE** (leer/fehlend matcht nicht, empirisch geprüft). Deshalb Worklist am
  Gerät → Bridge (11112); nicht auf „einfachere“ Alleinlösung über Orthanc umbauen, ohne diese Eigenschaft zu bedenken.
* Konfiguration der Bridge: `docker/bridge.conf` + Umgebung `OPENGDT_<SCHLÜSSEL>` (siehe `docker-compose.yml`). Neue Konfigurationsschlüssel,
  die der Stack braucht, dort ergänzen und in `.env.example` dokumentieren.
* Dateirechte: Der `init`-Dienst legt `./data` mit `PUID:PGID` an; die Bridge läuft nicht als root. Nie `user:` entfernen, sonst
  gehören die Dateien root und das PVS kann sie nicht löschen.
* **Zugangsdaten:** `docker/init.sh` erzeugt Passwörter (zufällig, einmalig) und schreibt `credentials.json` + `orthanc.json` in das Volume
  `orthanc-config` (Orthanc liest **ein** Konfigurationsverzeichnis – mehrere Pfade lehnt Orthanc ab, gegen 1.12 geprüft; Dateien ohne
  `.json` im Verzeichnis stören nicht). Die Bridge bekommt ihr Passwort nur als Datei (`orthanc_password_file`), nie als Umgebungsvariable.
  Passwörter dürfen nicht in Logs, `check-config`-Ausgabe, Repository oder `.env.example` stehen (Test `tests/cli.rs`). Passwortzeichen sind auf
  `A-Za-z0-9._-` beschränkt, weil `init.sh` die JSON-Datei mit `printf` schreibt (kein Escaping).
* **Startblock mit allen Zugangsdaten** (`docker/banner.sh`, Dienst `info`, auch per `docker/show-credentials.sh`): drei getrennte Bereiche
  (GNUmed, Orthanc, Ultraschallsystem), ausdrücklicher Nutzerwunsch, weil der Stack im Vordergrund (`up` ohne `-d`) laufen soll. Das Passwort
  steht deshalb im Klartext in den Docker-Logs – bewusst akzeptiert, nicht „reparieren“. Die Bridge-Zugangsdaten gehören **nicht** in den Block.
  Der Block erscheint erst, wenn die Bridge „healthy“ ist (Healthcheck in der Compose-Datei alle 3 s) und Port 80 offen ist. `info` läuft mit
  Host-Netzwerk, nur um die IP des Pi zu ermitteln. Was den GNUmed-*Client* betrifft, steht dort nur Belegtes bzw. ausdrücklich „ungeprüft“.
* `PVS_EXPORT_PREFIX` ist standardmäßig der **Host-Pfad** (`${PWD}/data/export`), nicht der Container-Pfad; sonst enthält die GDT-Rückmeldung
  Pfade, die GNUmed auf dem Pi nicht findet. Der Block warnt, falls trotzdem ein `/data/…`-Pfad aktiv ist.
* **Weboberfläche fest auf `127.0.0.1:80`** (ausdrücklicher Wunsch des Nutzers): nicht wieder variabel machen, nicht auf `0.0.0.0` öffnen.
* **GNUmed-Datenbank** (`gnumed-db`, `gnumed-bootstrap`, `docker/gnumed/`): Postgres **15** pinnen (16 hat kein `lc_ctype` in `pg_settings`, der
  Bootstrap von gnumed-server 22.28 bricht mit `IndexError` ab). Image `ubuntu:24.04`, Paket nur **entpackt** (`apt-get download` + `dpkg-deb -x`),
  weil die Abhängigkeit `postgresql` sonst einen zweiten Server installiert. Das Bootstrap-Skript findet Module relativ zu sich selbst und die
  Konfiguration wird per `sed` geändert → es läuft in einer Kopie (`/tmp/gnumed`). Nicht `gm-bootstrap_server` verwenden (Socket, `su postgres`).
  Die Testpatienten dürfen **nicht** entfernt werden (`bootstrap-test_data.conf` ist Voraussetzung der Updates, geprüft: sonst Abbruch bei v11).
  `gnumed-db` bekommt die Startoptionen `log_connections`, `log_disconnections`, `track_commit_timestamp` per `command:` (Nutzerwunsch, GNUmed-Hinweis); `track_commit_timestamp` braucht einen Neustart des Servers, bei `command:` kein Problem. Nicht in `postgresql.conf` im Volume ablegen.
  Vollständig = Datenbankkommentar `opengdt-bootstrap-fertig`; Staging-DBs `gnumed_v2…v21` werden danach gelöscht. Im offiziellen Postgres-Image ist
  `127.0.0.1` *im* Container `trust` – Passwortprüfungen deshalb über das Stack-Netzwerk testen (`pgc` in `smoke.sh`), nie per `exec psql -h 127.0.0.1`.
  Passwörter: Postgres-Admin und `gm-dbo` erzeugt (Volume `gnumed-secrets`); `any-doc/any-doc` bleibt Paketvorgabe (nur auf `127.0.0.1` erreichbar).
  Sandbox: Bauen nur mit `docker build --network host --build-arg BASE_IMAGE=mirror.gcr.io/library/ubuntu:24.04 -t opengdt-gnumed-bootstrap:local docker/gnumed`
  (Container haben dort kein DNS), danach `docker compose up -d` (nimmt das vorhandene Image).
* Nicht ändern ohne Rückfrage: veröffentlichte Ports, Authentifizierung, Image-Quellen.
* Nach Tests in der Sandbox aufräumen: `docker compose down -v; rm -rf data`.
