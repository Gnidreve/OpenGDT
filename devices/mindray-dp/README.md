# Mindray DP-70 / DP-50

Strukturierte Abschrift des **DICOM Conformance Statement** (P/N 046-001907-00, V1.0, 13.01.2011)
für DP-70, DP-70T, DP-70Vet, DP-50, DP-50T, DP-50Vet.

* Daten: [`dicom-conformance.json`](dicom-conformance.json) (SOP-Klassen, Transfer Syntaxes,
  Worklist-Schlüssel, Storage-Attribute, Statuscodes, Konfigurationsparameter)
* Quelle: [Mindray, H-046-001907-00 DP-70 DICOM Conformance Statement 1.0](https://www.mindray.com/content/dam/xpace/en_us/service-and-support/training-and-education/resource--library/technical--documents/dicom-conformance-guides-1/H-046-001907-00-DP-70-DICOM-Conformance-Statement-1.0.pdf)
  (Stand der Abschrift, Prüfsumme und Prüfstatus: Block `meta` im JSON)

### Profil aktualisieren

1. Neues PDF beschaffen; SHA-256, Größe und Seitenzahl in `meta.source` eintragen (`sha256sum datei.pdf`).
2. Daten neu abschreiben lassen (z. B. Claude das PDF lesen lassen), `meta.capture.extracted_at` (UTC) und
   `extracted_by` setzen, `review_status` auf `unreviewed`.
3. `cargo test --workspace`: Das Schema wird geprüft, die Bridge-Tests laufen gegen den neuen Stand.
4. Nach menschlicher Durchsicht `review_status = "reviewed"` mit `reviewed_by`/`reviewed_at`.
* Test gegen die Bridge: [`crates/bridge/tests/device_profiles.rs`](../../crates/bridge/tests/device_profiles.rs)

> **Stand 2011, Erstausgabe.** Ein Conformance Statement ersetzt keinen Test mit dem echten Gerät.

## Was das Gerät tut (Kurzfassung)

| Funktion | Rolle des Geräts | Wichtig für OpenGDT |
|---|---|---|
| Worklist (MWL) | SCU, Abfrage manuell oder automatisch (Broad Query), max. 10 000 Treffer, Association pro Abfrage | Standardabfrage filtert auf **eigenen AE-Titel** und **heutiges Datum** |
| Bildversand (C-STORE) | SCU, **eine Association je Untersuchung**, nacheinander; bei Fehlerstatus Abbruch | passt zur Bridge; Fehlerstatus nur bei echtem Archivfehler |
| Storage Commitment | SCU, Push Model, Rückkanal in **neuer Association zum Gerät** | **nicht unterstützt** → auf dem Gerät abschalten |
| Print, Media Storage | Print-SCU / USB-DICOMDIR | irrelevant |
| Zeichensatz | `ISO_IR 100` (Latin-1), optional GB18030 | passt zur Worklist der Bridge |

Bildformat laut Gerät: 8 Bit, Graustufen (`MONOCHROME2`) oder Farbe; Einzelbild (US Image),
Mehrbild/Cine (US Multi-frame) oder Secondary Capture. Patient-ID, Name, Accession Number und
Study Instance UID kommen **aus der Worklist** (oder per Hand – siehe Risiko unten).

## Was das für das Pilotprojekt bedeutet

### Behoben, weil der Abgleich mit dem Dokument es aufgedeckt hat
1. **Leere Worklist:** Die Abfrage des Geräts enthält Sequenzen als Return Keys (Referenced Study,
   Code Sequences) mit leeren Unterfeldern. Die Bridge wertete das als „muss existieren“ und lieferte
   **null Treffer**. Jetzt gelten rein leere Sequenzen als Return Key (Test `dp_worklist_query_finds_the_entry`).
2. **Implicit VR:** Unbekannte Sequenz-Tags wurden als Rohbytes gelesen und fälschlich als Suchwert
   gewertet. Sequenzen werden jetzt am Item-Marker erkannt.

### Einstellungen am Gerät (Checkliste)
Für den Docker-Stack gilt die Tabelle in [`docker/README.md`](../../docker/README.md): **Worklist → Bridge (Port 11112), Storage → Orthanc (Port 4242)**.
Grund: Orthancs Worklist-Plugin liefert nur bei exakt passendem Station-AE-Titel; das DP-50 fragt standardmäßig mit seinem eigenen (gegen Orthanc 1.12 geprüft).
* **Worklist-SCP** = Bridge (Host, Port `11112`, AE-Titel der Bridge). Station-AE-Schlüssel braucht
  keine Anpassung: Ist `station_ae` nicht konfiguriert, ignoriert die Bridge ihn.
* **Storage-SCP** = Bridge. **Compression Mode** bewusst wählen (siehe nächster Punkt).
* **Storage Commitment: „Associated Storage Service“ leer lassen.** Sonst fordert das Gerät nach jeder
  Untersuchung eine Bestätigung an, die die Bridge ablehnt (Symbol „nicht bestätigt“, Wiederholungen).
* **Allow multi-frame** nur einschalten, wenn Cine-Clips gewünscht sind (siehe unten).

### Offene Punkte, die nur ein Test am Gerät klärt
* **Welche Transfer Syntax wird gewählt?** Das Gerät listet *Implicit LE zuerst*. Die Bridge nimmt
  standardmäßig die erste angebotene → unkomprimiert → PNG (groß). Für kompakte **JPEG**-Dateien:
  `storage_transfer_syntaxes = jpeg-baseline, explicit-le` in der Konfiguration. Achtung: JPEG Baseline ist
  **verlustbehaftet**; ob das für die Praxis akzeptabel ist, ist eine Entscheidung des Anwenders,
  nicht der Software. Ob das Gerät die Liste in einem oder mehreren Contexts vorschlägt, ist unbekannt.
* **Compression Mode am Gerät** `JPEG Lossless`, `RLE`, `JPEG 2000`: Die Bridge archiviert die Originale,
  kann sie aber (noch) nicht in Bilddateien umwandeln → keine GDT-Rückmeldung für diese Bilder.
  Am Gerät **keine Kompression** oder **JPEG** wählen.
* **Cine-Clips:** Das Gerät kennt **kein MPEG-4**; Clips kommen als Multi-frame mit JPEG-/unkomprimierten
  Einzelbildern. Die Bridge macht daraus ein **AVI** (JPEG-Frames unverändert als MJPEG, unkomprimierte
  als unkomprimiertes AVI; Bildrate aus Frame Time). Mit `multiframe = first|all` stattdessen Einzelbilder.
  Offen: ob das **PVS** AVI abspielt. Mit JPEG-Transfer-Syntax (siehe oben) bleiben die Clips klein.
* **Patientenzuordnung:** Name, ID und Accession sind laut Dokument `MWL/USER`. Wer am Gerät einen
  Patienten von Hand anlegt, erzeugt eine ID, die das PVS nicht kennt – die GDT-Rückmeldung ist dann
  nicht zuordenbar. Betrieb ausschließlich über die Worklist.
* **Burned-in-Annotation:** Das Gerät schreibt Patientendaten ins Bild (`YES`). Exportierte Bilder
  tragen sie also mit; für Datenschutz/Pseudonymisierung relevant.
