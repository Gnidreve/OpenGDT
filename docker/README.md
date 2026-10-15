# Docker-Stack (Entwicklung / Test)

Ein Befehl startet alles und verdrahtet es: **Orthanc** (DICOM-Server, Bildspeicher), die **OpenGDT-Bridge** und die **GNUmed-Datenbank**
(PostgreSQL). Der **GNUmed-Client läuft nicht im Stack**, sondern direkt auf dem Pi: er verbindet sich mit der Datenbank auf `localhost:5432`
und tauscht GDT-Dateien über den Ordner `./data` aus.

```
Ultraschall DP-50 ──Worklist (DICOM :11112)──────────────▶ bridge ◀──GDT-Dateien──▶ PVS (GNUmed, auf dem Pi)
                 └─Bilder    (DICOM :4242)──▶ orthanc ──REST──▶ bridge ──▶ Bilddateien + GDT-Rückmeldung
```

## Starten

```sh
cp .env.example .env              # optional anpassen: Zeitzone, Pfad für das PVS (siehe Datei)
docker compose up --build         # im Vordergrund: Logs live im Terminal; erster Build auf dem Pi dauert einige Minuten
```

Sobald alles läuft, druckt der Stack **einen Block mit allen Zugangs- und Verbindungsdaten**, getrennt nach drei Bereichen:
**1) GNUmed**, **2) Orthanc-Weboberfläche**, **3) Ultraschallsystem** (IP des Pi, Ports, AE-Titel, die du am Gerät einträgst). Den Block gibt es
jederzeit noch einmal mit `docker/show-credentials.sh`. Er enthält das Passwort im Klartext und steht damit auch in den Docker-Logs
(`docker compose logs`); das ist so gewollt, damit du morgen nichts suchen musst.

Weitere Befehle (in einem zweiten Terminal):

```sh
docker/smoke.sh                   # prüft Dienste, Verbindungen und GDT → Worklist
docker/send-test-patient.sh       # legt eine Test-Anforderung ab (Patient 4711, Müller, Jürgen)
```

Beenden: `Strg+C` im Terminal bzw. `docker compose down`. Alles zurücksetzen: `docker compose down -v && rm -rf data`.
Im Hintergrund starten geht mit `docker compose up -d --build` (Logs dann mit `docker compose logs -f`).

### Mehrere Ultraschallgeräte

Alle Geräte tragen **dieselben Serverdaten** ein (IP des Pi, Port 11112/4242, AE-Titel `OPENGDT`/`ORTHANC`). Nur der **eigene AE-Titel** jedes
Geräts unterscheidet sich (z. B. `DP50-1`, `DP50-2`). Der Stack prüft AE-Titel nicht und hat keine Gerätezuordnung: Worklist-Einträge gelten für jedes
Gerät, Bilder werden nach Patient/Untersuchung abgelegt, nicht nach Gerät. Sinnvoll nur, solange `DEVICE_AE` in `.env` leer bleibt.

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

**Getestet** (x86_64, Docker 29, echtes `orthancteam/orthanc`-Image, `pynetdicom` als Gerät): kompletter Ablauf mit
`docker compose up`, Worklist von der Bridge, Bilder (JPEG, unkomprimiert, Cine als Mehrbild) an Orthanc, Abholung per REST,
GDT-Rückmeldung, AVI-Export; die Abfrage des DP-50 aus dem Conformance Statement gegen Orthanc und Bridge.

**Nicht getestet:**
* **Raspberry Pi / arm64.** `orthancteam/orthanc:latest` hat laut Registry-Manifest ein arm64-Image; **gelaufen ist es nur auf
  x86_64**. Die Bridge wird auf dem Pi selbst gebaut (erster Build dauert dort einige Minuten).
* **Das echte DP-50**, **der GNUmed-Client** (siehe unten), **das Bauen des Bootstrap-Images auf arm64** und die Sommerzeit-Umstellung.

## GNUmed-Datenbank im Stack

* Dienst `gnumed-db`: PostgreSQL **15** (nicht 16: der Bootstrap von GNUmed 22 bricht dort ab, geprüft), erreichbar **nur** auf dem Pi unter
  `localhost:5432`. Dienst `gnumed-bootstrap`: legt beim **ersten Start** die Datenbank `gnumed_v22` an und tut danach nichts mehr
  (Meldung „nichts zu tun“). Der erste Aufbau dauert mehrere Minuten (hier auf x86_64 gut 4; auf dem Pi länger, **nicht gemessen**), weil das
  Schema von Version 2 bis 22 aufgebaut wird. Der Infoblock erscheint deshalb beim ersten Start erst danach; der Fortschritt steht im Log.
* Die Daten liegen im Volume `gnumed-db` und überleben `docker compose down`; `down -v` löscht sie. Ein abgebrochener Aufbau wird beim
  nächsten Start automatisch neu gemacht (Marker `opengdt-bootstrap-fertig` am Datenbankkommentar).
* Client und Datenbank müssen zusammenpassen: Datenbank **v22** gehört zum GNUmed-Client **1.8.x**. Welche Version auf dem Pi installiert wird,
  ist ungeprüft – bei einer anderen Hauptversion meldet der Client einen Versionskonflikt.
* Zugang: Datenbank `gnumed_v22`, Host `localhost`, Port `5432`. Benutzer **`any-doc`**, Passwort **`any-doc`** (Paketvorgabe, am Datenbankserver
  getestet; die Anmeldung im Client selbst ist ungetestet). Der Datenbank-Eigentümer `gm-dbo` hat ein **erzeugtes** Passwort (steht im
  Infoblock; die Paketvorgabe `gm-dbo` wird abgelehnt). Der Postgres-Administrator hat ein erzeugtes Passwort, das nur im Volume `gnumed-secrets` liegt.
* Das GNUmed-Paket legt **synthetische Testpatienten** an; die Update-Skripte setzen sie voraus (ohne sie bricht der Aufbau ab, geprüft).
* Quellen: [README.Debian des Pakets gnumed-server](https://metadata.ftp-master.debian.org/changelogs/main/g/gnumed-server/gnumed-server_22.33-1_README.Debian),
  außerdem das entpackte Paket `gnumed-server` 22.28 (Ubuntu 24.04), aus dem das Image gebaut wird (amd64 + arm64).
* Keine Verbindung zu den Orthanc-Zugangsdaten. Ports: `80` (Orthanc), `4242`, `11112` (DICOM), `5432` (GNUmed-Datenbank).

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
