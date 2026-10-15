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

Weboberfläche: <http://localhost:8042> (nur vom Pi selbst erreichbar). Dort sieht man empfangene Untersuchungen und Bilder.
Nützlich beim Debuggen: `curl localhost:8042/changes?last`, `curl localhost:8042/studies`.

## Wenn etwas nicht klappt

| Symptom | Prüfen |
|---|---|
| Gerät zeigt **leere Worklist** | `docker compose logs bridge`: steht dort „Worklist-Abfrage von … 1 Treffer“? Wenn „0 Treffer“: Zeitzone (`UTC_OFFSET_MINUTES`), Gerät fragt nach Datum/Station? Eintrag vorhanden (`ls data/worklists`)? Worklist-Server am Gerät auf Port **11112**? |
| Gerät meldet **Verbindung fehlgeschlagen** | Pi-IP/Port, Firewall (`4242`, `11112`), `docker compose ps` (alles „Up“?), `docker/smoke.sh` |
| Bilder landen in Orthanc, aber **keine GDT-Datei** | Orthanc meldet eine Untersuchung erst `StableAge` (5 s) nach dem letzten Bild; dann holt die Bridge sie („Orthanc: Untersuchung … übernommen“). Danach `settle_seconds` (3 s). Log der Bridge ansehen. |
| GDT-Datei erscheint, **PVS findet Bilder nicht** | `PVS_EXPORT_PREFIX` falsch (Container-Pfad statt Host-Pfad?), Dateirechte (`PUID`/`PGID`) |
| Bridge-Log: „Orthanc nicht erreichbar“ | Orthanc noch am Starten (normal für ~10 s) oder abgestürzt: `docker compose logs orthanc` |

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

## GNUmed – offene Punkte

Ob und wie GNUmed den GDT-Ablauf unterstützt (Gerät starten, Satzarten und Felder für Auftrag/Ergebnis, Verweis oder Messwerte/Bilder),
ist **noch nicht geklärt** – die Dokumentation ist von hier aus nicht erreichbar, und eine Websuche ergab dazu nichts Belastbares.
Bis dahin arbeitet die Bridge mit definierten Testdateien (`docker/testdata/PATIN.GDT`, Satzarten und Felder siehe
[`crates/gdt/CLAUDE.md`](../crates/gdt/CLAUDE.md), Abschnitt „Ungeprüft“). Satzart der Rückmeldung (`gdt_result_type`) und Dateiname
(`GDT_RESULT_FILENAME`) sind konfigurierbar.

## Sicherheit

Dieser Stack ist für **Entwicklung im Praxisnetz**, nicht für das Internet: Orthanc läuft ohne Benutzerverwaltung (Weboberfläche
nur auf `127.0.0.1`), DICOM ist unverschlüsselt, Port `4242`/`11112` sind im LAN offen. Vor echtem Praxisbetrieb: Zugang,
Verschlüsselung, Backups und Datenschutz (siehe [`concept/`](../concept/README.md#6-offene-punkte-und-risiken)).
