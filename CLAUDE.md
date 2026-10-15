# CLAUDE.md – OpenGDT

Anweisungen für Agenten. Menschen lesen die `README.md` im jeweiligen Ordner.

## Projekt in einem Satz
Rust-Bridge zwischen DICOM-Ultraschallgeräten und deutschen PVS (GDT): GDT-Anforderung →
DICOM-Worklist; DICOM-Bilder → JPEG/MP4/PNG/PDF-Dateien + GDT-Rückmeldung.

## Regeln
1. **Dokumentationsstruktur = Codestruktur.** Jeder Ordner mit Code oder Konzept hat eine
   `README.md` (für Menschen: Zweck, Benutzung, Grenzen) und eine `CLAUDE.md` (für Agenten:
   Invarianten, Fallen, Entscheidungen). Es gibt bewusst **keinen** zentralen `docs/`-Ordner.
   Ändert sich Code, ändert sich die Doku im selben Ordner im selben Commit.
   Die Doku liegt auf Komponentenebene (Crate-Ordner); `src/` und `tests/` innerhalb eines Crates
   haben keine eigenen Doku-Dateien.
2. **Nur `std`.** Keine externen Crates (Entscheidung des Nutzers). Vorschläge für Ausnahmen
   als Rückfrage formulieren, nicht einbauen. Dev-only-Werkzeuge außerhalb von Cargo
   (z. B. `tests/interop`) sind erlaubt, solange sie das Projekt nicht berühren.
3. **Verifizieren vor „fertig“:** `cargo fmt --all`, `cargo clippy --workspace --all-targets`
   (warnungsfrei), `cargo test --workspace`; bei Änderungen an DICOM/GDT-Ausgabe zusätzlich
   `tests/interop/run.sh`; bei Änderungen am Orthanc-Anschluss oder am Stack zusätzlich `tests/orthanc/run.sh` und `docker/smoke.sh`.
   Coverage-Stand (Zeilen, gesamt): ~95 %; übrig sind im Kern reine I/O-Fehlerzweige
   (`accept()`-Fehler in `dicom/scp.rs`, Schreibfehler-Fortsetzungen, `bridge/watcher.rs`).
4. **Ehrlichkeit über Spezifikationslücken:** Was nicht gegen die offizielle Spezifikation
   geprüft ist, steht als „Ungeprüft“ in der CLAUDE.md des Crates (aktuell `crates/gdt`).
   Nichts als Fakt behaupten, was aus dem Gedächtnis stammt.
5. Sprache: Doku und Log-Meldungen Deutsch, Bezeichner im Code Englisch.
6. Patientendaten gehören nie in Logs auf INFO-Level über Kennung/Name hinaus, nie in Testdaten
   echter Personen. Testdaten sind synthetisch.

## Karte
* [`concept/`](concept/README.md) – Konzeptpapier (Warum, Architektur, Risiken)
* [`crates/`](crates/README.md) – Code (Workspace)
* [`devices/`](devices/README.md) – Gerätedaten aus Herstellerdokumenten (Pilot: Mindray DP-50)
* [`docker/`](docker/README.md) – Compose-Stack (Orthanc + Bridge), PVS (GNUmed) liegt außerhalb und tauscht GDT-Dateien über `./data`
* [`tests/orthanc/`](tests/orthanc/README.md) – Test gegen ein echtes Orthanc
* [`tests/interop/`](tests/interop/README.md) – Interop-Test gegen pynetdicom

## Offene Entscheidungen (beim Nutzer)
Siehe Abschnitt „Offene Fragen“ in der Sitzungszusammenfassung bzw. `README.md` → Status.
