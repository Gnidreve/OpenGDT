# OpenGDT

Eine schlanke, **abhängigkeitsfreie** Bridge zwischen DICOM-Ultraschallgeräten und deutschen
Praxisverwaltungssystemen (PVS) mit GDT-Schnittstelle. Geschrieben in Rust, nur mit der
Standardbibliothek.

```
 PVS ──GDT-Datei──▶ OpenGDT ──DICOM-Worklist──▶ Ultraschallgerät
 PVS ◀─GDT-Datei── OpenGDT ◀──DICOM-Bilder─────  Ultraschallgerät
         + Bilddateien (JPEG/MP4/PNG/PDF) im Netzwerkordner
```

* Der Arzt startet im PVS eine Untersuchung → das Gerät findet den Patienten in seiner
  **Worklist**, nichts wird abgetippt.
* Nach der Untersuchung sendet das Gerät die Bilder → OpenGDT archiviert die Originale,
  konvertiert sie und trägt die Bilder per **GDT-Rückmeldung** in die Patientenakte ein.

Hintergrund, Marktkontext und Risiken: [`concept/`](concept/README.md).

## Status

**Vorabversion (0.1) – Ende-zu-Ende lauffähig, aber noch nie an einem echten Gerät oder
PVS getestet.**

| Bereich | Stand |
|---|---|
| GDT lesen/schreiben (Zeichensätze, Längen) | ✅ implementiert und getestet; einzelne Feldkennungen/Satzarten **ungeprüft** gegen die Spezifikation (→ [`crates/gdt`](crates/gdt/CLAUDE.md)) |
| DICOM: C-ECHO, C-FIND (Worklist), C-STORE | ✅ implementiert; Interop mit `pynetdicom` bestanden |
| Konvertierung: JPEG/MP4 durchreichen, PNG, PDF | ✅ implementiert; JPEG-2000/RLE/Cine-aus-Einzelframes ❌ |
| Mehrfach-Bilder → eine GDT-Rückmeldung, Neustart-fest | ✅ |
| TLS, Windows-Dienst, Signal-Handling, MPPS | ❌ |
| Rechtliche Einordnung (MDR), Datenschutzkonzept | ❌ offen, siehe Konzept |

**Nicht in einer Praxis einsetzen**, bevor Satzarten/Feldkennungen mit dem Ziel-PVS abgestimmt,
ein Rauchtest am Gerät gelaufen und die MDR-Frage geklärt ist.

## Schnellstart

```sh
cargo build --release
cp crates/bridge/opengdt.conf.example opengdt.conf     # anpassen
./target/release/opengdt-bridge check-config opengdt.conf
./target/release/opengdt-bridge run opengdt.conf
```

Im Gerät als „Worklist-Server“ und „Storage-Server“ eintragen: Host der Bridge, Port `11112`,
AE-Titel `OPENGDT`. Test der Verbindung: `opengdt-bridge echo HOST:11112`.
Das PVS legt Anforderungen als `*.gdt` in `gdt_import_dir` ab und liest Rückmeldungen
aus `gdt_export_dir`.

Ohne PVS lässt sich alles mit einer von Hand geschriebenen GDT-Datei ausprobieren, siehe
[`concept/`](concept/README.md#4-lizenzfreies-testen).

## Aufbau des Repositories

Jeder Ordner enthält eine `README.md` (für Menschen) und eine `CLAUDE.md` (für KI-Agenten).

```
concept/            Konzeptpapier
crates/
  gdt/              GDT-Parser/-Writer
  dicom/            DICOM-Stack: Datensätze, Upper Layer, ECHO/FIND/STORE
  worklist/         GDT-Patient → Worklist-Eintrag, Ablage, Matching
  store/            Archiv der empfangenen Instanzen
  convert/          DICOM-Pixeldaten → Dateien fürs PVS
  bridge/           Binary: Konfiguration, Watcher, Pipeline, CLI
tests/interop/      Optionaler Test gegen pynetdicom (Python, nur Dev)
```

## Entwickeln

```sh
cargo fmt --all
cargo clippy --workspace --all-targets
cargo test --workspace        # Unit-, Loopback- und Ende-zu-Ende-Tests, ohne externe Hilfsmittel
tests/interop/run.sh          # optional, braucht Python + Internet (nur für diesen Test)
```

Projektregel: **keine externen Crates.**

## Lizenz

[MIT](LICENSE)
