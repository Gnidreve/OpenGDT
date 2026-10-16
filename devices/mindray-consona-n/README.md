# Mindray Consona N6 / N7 / N8 / N9

Strukturierte Abschrift des **DICOM Conformance Statement** (P/N 046-024940-00, V3.0, 13.01.2023, 331 Seiten)
für die Consona-N-Reihe. Zweites Profil neben dem Piloten [`mindray-dp`](../mindray-dp/README.md) (DP-70/DP-50).

* Daten: [`dicom-conformance.json`](dicom-conformance.json) (SOP-Klassen, Transfer Syntaxes, Worklist-Schlüssel, MPPS,
  Storage Commitment, Sicherheit, Storage-Attribute, Konfigurationsparameter)
* Quelle: [Mindray, Consona N6 and N9 DICOM Conformance Statement V3.0](https://www.mindray.com/content/dam/xpace/en_us/service-and-support/training-and-education/resource--library/technical--documents/dicom-conformance-guides-1/Consona-N6-and-N9-DICOM-CONFORMANCE-STATEMENT-V3.0.pdf)
  (Prüfsumme, Zeitstempel und Prüfstatus: Block `meta` im JSON). Das PDF liegt nicht im Repository.
* Test gegen die Bridge: [`crates/bridge/tests/device_profiles.rs`](../../crates/bridge/tests/device_profiles.rs) (läuft für jedes Profil)

> Abgeschrieben sind Netzwerk (Abschnitt 4), Media Storage, Zeichensätze, Sicherheit und die IOD-Tabellen.
> **Nicht abgeschrieben:** die Structured-Report-Templates (Anhang A–G, Seiten 95–331); die Bridge wertet SR nicht aus.
> Stand der Abschrift: `unreviewed`, nie an einem echten Gerät geprüft.

## Was dieses Gerät anders macht als das DP-50

| Thema | DP-50 (2011) | Consona N6–N9 (2023) | Folge für OpenGDT |
|---|---|---|---|
| Worklist, Standardabfrage | Scheduled Station AE = **eigener AE-Titel**, Datum heute | Scheduled Station AE **leer**, Datum heute, Modality US | Bridge: leerer Schlüssel trifft immer, mit und ohne konfiguriertes `station_ae` (Test `default_query_with_empty_station_ae…`). Gegen das Worklist-Plugin von Orthanc ungeprüft |
| Worklist, Suche | Platzhalter `*` | Platzhalter `?` und `*` | Bridge unterstützt beides |
| **MPPS** | nicht vorhanden | **ja** (N-CREATE IN PROGRESS beim Start, N-SET COMPLETED/DISCONTINUED) | Bridge lehnt den Dienst ab. Am Gerät **keinen MPPS-Dienst** auf Bridge/Orthanc einrichten |
| Storage Commitment | ja (SCU) | ja (SCU, nur nach „End Exam“) | wie beim DP-50: „Associated Storage Service“ leer lassen |
| Bildkompression | JPEG Baseline, JPEG Lossless, RLE, JPEG 2000 | dasselbe **plus MPEG2 MP@ML** (Mehrbild mit Doppler-Audio) | Die Bridge wandelt nur unkomprimiert und **JPEG Baseline** um. Alles andere bleibt im Archiv (kein Bild im PVS). Compression Mode am Gerät entsprechend stellen, „Doppler Audio“ auf None |
| Weitere Storage-Klassen | – | Comprehensive SR, Encapsulated PDF, Raw Data, Enhanced US Volume | werden angenommen und archiviert, aber **nicht umgewandelt** |
| Query/Retrieve | – | Study Root FIND/MOVE (SCU) | irrelevant, Bridge bietet es nicht an |
| Zeichensatz | `ISO_IR 100`, GB18030 | `AUTO`, bei Deutsch/Englisch **UTF-8 (`ISO_IR 192`)** | Bridge liest beide beim Empfang; die Worklist-Antwort der Bridge ist `ISO_IR 100`: **Umlaute am Gerät prüfen** |
| Sicherheit | keine | TLS 1.2 optional (aus), sonst keine Prüfung des AE-Titels/der IP | TLS am Gerät aus lassen (Bridge/Orthanc im Stack können kein TLS) |
| PDU-Größe | 16 384 – 65 536 | gleich | – |

## Einstellungen am Gerät (Checkliste, gleiche Serverdaten wie beim DP-50)

Für den Docker-Stack gilt die Tabelle in [`docker/README.md`](../../docker/README.md): **Worklist → Bridge (Port 11112, AE `OPENGDT`),
Storage → Orthanc (Port 4242, AE `ORTHANC`)**.

* Im Dialog „DICOM Service“ den Worklist- und den Storage-Dienst anlegen und je als **Default** setzen, dann fragt das Gerät
  die Worklist automatisch beim Öffnen ab. Ohne Default muss man die Schlüssel von Hand eintragen.
* **Storage:** Compression Mode auf JPEG Lossy bzw. unkomprimiert; „Doppler Audio“ = None; „Allow Multiframe“ nur für Cine-Clips.
  Welche Werte der Compression Mode anbietet, steht nicht im Dokument.
* **Storage Commitment:** „Associated Storage Service“ leer. **MPPS:** keinen Dienst auf die Bridge zeigen lassen.
* **TLS** bei allen Diensten aus.
* Senden: automatisch nach „End Exam“ (Voreinstellung „Sending/printing after End Exam“), per Sendetaste während der Untersuchung
  oder manuell aus der Bildauswahl.

## Profil aktualisieren

Siehe `capture.update_procedure` im JSON und [`../CLAUDE.md`](../CLAUDE.md). Die CDN von mindray.com weist `curl` ohne Browser-Kennung
mit 403 ab (mit gewöhnlichem `User-Agent` kommt das PDF).
