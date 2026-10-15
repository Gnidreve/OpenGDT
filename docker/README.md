# Docker-Stack (Entwicklung / Test)

Ein Befehl startet alles und verdrahtet es: **Orthanc** (DICOM-Server, Bildspeicher) und die **OpenGDT-Bridge**.
Das **PVS (GNUmed) läuft nicht im Stack**, sondern auf dem Host und tauscht GDT-Dateien über den Ordner `./data` aus.

```
Ultraschall DP-50 ──Worklist (DICOM :11112)──────────────▶ bridge ◀──GDT-Dateien──▶ PVS (GNUmed, auf dem Pi)
                 └─Bilder    (DICOM :4242)──▶ orthanc ──REST──▶ bridge ──▶ Bilddateien + GDT-Rückmeldung
```

## Starten

```sh
cp .env.example .env              # anpassen: Zeitzone, AE-Titel des Geräts, Pfad für das PVS (siehe Datei)
docker compose up -d --build      # der erste Build auf dem Pi dauert einige Minuten (Rust wird kompiliert)
docker/smoke.sh                   # prüft Dienste, Verbindungen und GDT → Worklist
docker/send-test-patient.sh       # legt eine Test-Anforderung ab (Patient 4711, Müller, Jürgen)
docker compose logs -f bridge orthanc
```

Beenden: `docker compose down`. Alles zurücksetzen: `docker compose down -v && rm -rf data`.

## Am Mindray DP-50 einstellen

Das Gerät lässt Worklist- und Storage-Server getrennt eintragen (Conformance Statement, Abschnitt 8.2).

| Dienst am Gerät | Server | Port | AE-Titel (Called AE) |
|---|---|---|---|
| **Worklist** | IP des Pi | **11112** | `OPENGDT` |
| **Storage** | IP des Pi | **4242** | `ORTHANC` |
| Storage Commitment | **nicht einrichten** (leer lassen) | | |

Der lokale AE-Titel des Geräts ist frei wählbar. `DEVICE_AE` in `.env` leer lassen, dann gilt jeder Worklist-Eintrag für jedes Gerät.

**Warum die Worklist nicht auch über Orthanc (4242)?** Getestet gegen Orthanc 1.12: Dessen Worklist-Plugin liefert einen
Eintrag nur, wenn der Station-AE-Titel im Eintrag **genau** dem der Abfrage entspricht. Das DP-50 fragt standardmäßig mit
seinem eigenen AE-Titel; ein Eintrag mit leerem oder fehlendem Titel wird dann **nicht** gefunden (Ergebnis: leere Liste).
Die Worklist der Bridge ignoriert den Titel, solange `DEVICE_AE` leer ist. Wer trotzdem alles über Orthanc laufen lassen
will: `DEVICE_AE` auf den AE-Titel des Geräts setzen, dann passt es auch dort.

## Ordner (`./data`, gehört dem Benutzer `PUID:PGID` aus `.env`)

| Ordner | Inhalt | Wer schreibt |
|---|---|---|
| `data/gdt-in` | GDT-Anforderungen (`*.gdt`) | PVS → Bridge liest und verschiebt nach `gdt-in/processed` |
| `data/gdt-out` | GDT-Rückmeldungen mit Bildverweisen | Bridge → PVS liest |
| `data/export` | konvertierte Bilder/Videos (`<PatientID>/<Datum>_<Accession>/…`) | Bridge → PVS öffnet sie |
| `data/worklists` | Worklist-Einträge (`*.wl`) | Bridge → Orthanc-Plugin liest sie ebenfalls |
| `data/archive` | DICOM-Originale (Quelle der Wahrheit) | Bridge |
| Volume `orthanc-db` | Orthancs Datenbank | Orthanc |

`PVS_EXPORT_PREFIX` in `.env` muss der Pfad sein, **unter dem das PVS die Bilder sieht**, also bei GNUmed auf dem Pi der
absolute Host-Pfad, z. B. `/home/pi/OpenGDT/data/export`. Er steht so in der GDT-Rückmeldung.

## Orthanc ansehen

Weboberfläche: <http://localhost> (Port **80**, fest, und nur auf `127.0.0.1`, also nur im Browser des Pi selbst erreichbar).
Dort sieht man empfangene Untersuchungen und Bilder. Der Browser fragt nach Benutzer und Passwort:

```sh
docker/show-credentials.sh        # zeigt URL, Benutzer "praxis" und das Passwort
```

Nützlich beim Debuggen: `curl -u praxis:PASSWORT localhost/changes?last`, `curl -u praxis:PASSWORT localhost/studies`.
Port 80 darf auf dem Pi nicht anderweitig belegt sein (z. B. nginx/apache), sonst startet `orthanc` nicht.

## Zugangsdaten

Orthanc ist mit Benutzername und Passwort geschützt (HTTP Basic). Es gibt zwei Benutzer:

| Benutzer | Wofür | Wo das Passwort liegt |
|---|---|---|
| `praxis` | Weboberfläche im Browser | Volume `orthanc-config` (anzeigen: `docker/show-credentials.sh`) |
| `bridge` | die Bridge (holt Untersuchungen ab) | Volume `bridge-secrets`, nur die Bridge sieht es |

Die Passwörter werden **beim ersten Start zufällig erzeugt** (24 Zeichen), stehen nicht im Repository und nicht in `.env` und
bleiben bei Neustarts erhalten. Selbst festlegen: `ORTHANC_ADMIN_PASSWORD` / `ORTHANC_BRIDGE_PASSWORD` in `.env`
(12–64 Zeichen aus `A-Z a-z 0-9 . _ -`). **Ändern geht in zwei Schritten** (Orthanc liest die Zugangsdaten nur beim Start):

```sh
docker compose up -d                    # Init-Schritt schreibt die neuen Zugangsdaten
docker compose restart orthanc bridge   # erst jetzt gelten sie
```

Ungültige Passwörter in `.env` lassen den Init-Schritt scheitern (`docker compose logs init` nennt den Grund). Alles zurücksetzen:
`docker compose down -v` (löscht auch die Orthanc-Datenbank!).

**Was das schützt und was nicht:** geschützt ist die Weboberfläche/REST von Orthanc (und damit der Zugriff auf alle Bilder).
**Nicht** geschützt ist DICOM: Orthancs Port `4242` und der DICOM-Port `11112` der Bridge nehmen weiterhin Verbindungen
aus dem Praxisnetz an (DICOM kennt hier keine Anmeldung, wie bei jedem Ultraschallgerät); die Passwörter liegen im Klartext in
den Docker-Volumes (Orthanc-Vorgabe).

## Wenn etwas nicht klappt

| Symptom | Prüfen |
|---|---|
| Gerät zeigt **leere Worklist** | `docker compose logs bridge`: steht dort „Worklist-Abfrage von … 1 Treffer“? Wenn „0 Treffer“: Zeitzone (`UTC_OFFSET_MINUTES`), Gerät fragt nach Datum/Station? Eintrag vorhanden (`ls data/worklists`)? Worklist-Server am Gerät auf Port **11112**? |
| Gerät meldet **Verbindung fehlgeschlagen** | Pi-IP/Port, Firewall (`4242`, `11112`), `docker compose ps` (alles „Up“?), `docker/smoke.sh` |
| Bilder landen in Orthanc, aber **keine GDT-Datei** | Orthanc meldet eine Untersuchung erst `StableAge` (5 s) nach dem letzten Bild; dann holt die Bridge sie („Orthanc: Untersuchung … übernommen“). Danach `settle_seconds` (3 s). Log der Bridge ansehen. |
| GDT-Datei erscheint, **PVS findet Bilder nicht** | `PVS_EXPORT_PREFIX` falsch (Container-Pfad statt Host-Pfad?), Dateirechte (`PUID`/`PGID`) |
| Bridge-Log: „Orthanc nicht erreichbar“ | Orthanc noch am Starten (normal für ~10 s) oder abgestürzt: `docker compose logs orthanc` |
| Bridge-Log: „Orthanc verweigert den Zugriff“ | Passwort des Benutzers `bridge` passt nicht zu Orthanc: `ORTHANC_BRIDGE_PASSWORD` in `.env` geändert? Dann `docker compose up -d && docker compose restart orthanc bridge` |
| `orthanc` startet nicht, Log nennt Port 80 | Auf dem Pi läuft schon ein Webserver auf Port 80 → stoppen/deaktivieren |

## Was getestet ist – und was nicht

**Getestet** (x86_64, Docker 29, echtes `jodogne/orthanc-plugins`-Image, `pynetdicom` als Gerät): kompletter Ablauf mit
`docker compose up`, Worklist von der Bridge, Bilder (JPEG, unkomprimiert, Cine als Mehrbild) an Orthanc, Abholung per REST,
GDT-Rückmeldung, AVI-Export; die Abfrage des DP-50 aus dem Conformance Statement gegen Orthanc und Bridge.

**Nicht getestet:**
* **Raspberry Pi / arm64.** Ob `jodogne/orthanc-plugins:latest` ein arm64-Image bietet, ist ungeprüft. Falls nicht: anderes
  Image in `.env` (`ORTHANC_IMAGE=…`); Pfad des Worklist-Plugins in `docker/orthanc/orthanc.json` dann ggf. anpassen.
* **Das echte DP-50**, **GNUmed** (siehe unten) und die Sommerzeit-Umstellung.
* Der Plugin-Pfad im Image ist `/usr/local/share/orthanc/plugins/libModalityWorklists.so` (gegen das Image geprüft, nicht
  gegen Orthanc-Debian-Pakete, wo er anders liegt).

## GNUmed und die Zugangsdaten

GNUmed gehört **nicht** zum Stack und hat **keine** Verbindung zu den Orthanc-Zugangsdaten. Nach dem, was sich recherchieren ließ
(Debian-Paketdokumentation, nicht selbst ausprobiert):

* GNUmed ist ein Desktop-Programm (der Client) und hat selbst **keinen Server-Port**. Es verbindet sich mit einer **PostgreSQL-Datenbank**,
  Standardport **5432** (bei anderem Port setzt man `GM_DB_PORT`). Die Datenbank legt `gm-bootstrap_server` an (**Achtung:** löscht
  vorhandene GNUmed-Datenbanken). Quelle: [README.Debian des Pakets gnumed-server](https://metadata.ftp-master.debian.org/changelogs/main/g/gnumed-server/gnumed-server_22.33-1_README.Debian).
* Angemeldet wird im Client mit einem GNUmed-Benutzer; in älteren Anleitungen ist das Standardkonto `any-doc` (siehe
  [diese Anleitung](https://www.ghacks.net/?p=18370)) – ob das für die aktuelle Version noch gilt, ist ungeprüft. Maßgeblich ist die
  GNUmed-Dokumentation.
* Es gibt keine Überschneidung mit den Ports des Stacks (`80`, `4242`, `11112` – PostgreSQL liegt auf `5432`).

## GNUmed – offene Punkte

Ob und wie GNUmed den GDT-Ablauf unterstützt (Gerät starten, Satzarten und Felder für Auftrag/Ergebnis, Verweis oder Messwerte/Bilder),
ist **noch nicht geklärt** – die Dokumentation ist von hier aus nicht erreichbar, und eine Websuche ergab dazu nichts Belastbares.
Bis dahin arbeitet die Bridge mit definierten Testdateien (`docker/testdata/PATIN.GDT`, Satzarten und Felder siehe
[`crates/gdt/CLAUDE.md`](../crates/gdt/CLAUDE.md), Abschnitt „Ungeprüft“). Satzart der Rückmeldung (`gdt_result_type`) und Dateiname
(`GDT_RESULT_FILENAME`) sind konfigurierbar.

## Sicherheit

Dieser Stack ist für **Entwicklung im Praxisnetz**, nicht für das Internet: Orthancs Weboberfläche ist passwortgeschützt und nur
auf `127.0.0.1` erreichbar, aber DICOM ist unverschlüsselt und unauthentifiziert, Port `4242`/`11112` sind im LAN offen. Vor echtem Praxisbetrieb: Zugang,
Verschlüsselung, Backups und Datenschutz (siehe [`concept/`](../concept/README.md#6-offene-punkte-und-risiken)).
