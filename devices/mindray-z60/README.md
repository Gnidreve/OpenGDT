# Mindray Z6 / Z60 / Z80 / Z8

Strukturierte Abschrift des **DICOM Conformance Statement** (P/N 046-017363-00, V1.0, 05.06.2019, 201 Seiten) für die Z-Reihe:
Z6, Z6T/S/W, Z6 Pro/Expert/Vet, **Z60**, Z60T/S/W, Z60 Pro/Expert/Vet, Z80/Z80S/Pro/Expert, Z8/Z8S/Pro/Expert.

* Daten: [`dicom-conformance.json`](dicom-conformance.json) (SOP-Klassen, Transfer Syntaxes, Worklist-Schlüssel, MPPS, Storage Commitment, Storage-Attribute, Konfigurationsparameter)
* Quelle: [Mindray, H-046-017363-00 DICOM Conformance Statement V1.0](https://www.mindray.com/content/dam/xpace/en_us/service-and-support/training-and-education/resource--library/technical--documents/dicom-conformance-guides-1/H-046-017363-00-DICOM-Conformance-Statement-.pdf)
  (Prüfsumme, Zeitstempel und Prüfstatus: Block `meta` im JSON). Das PDF liegt nicht im Repository.
* Test gegen die Bridge: [`crates/bridge/tests/device_profiles.rs`](../../crates/bridge/tests/device_profiles.rs) (läuft für jedes Profil)

> Nicht abgeschrieben: Print-Details, die MPPS-Attributtabelle (Tabelle 36) und die Query/Retrieve-Attribute.
> Stand der Abschrift: `unreviewed`, nie an einem echten Gerät geprüft.

## Wichtig: Z60 Vet und Z6 Vet

Laut Fußnoten der Tabellen 1 und 3 haben **Z60 Vet und Z6 Vet keinen Worklist-, MPPS-, Query/Retrieve- und SR-Dienst**. Bei diesen Modellen
entfällt der ganze Weg „GDT → Worklist → Gerät“; es bleibt nur der Bildversand. Alle anderen Z-Modelle haben die Dienste.

## Unterschiede zu DP-50 und Consona N

| Thema | DP-50 | Consona N6–N9 | Z60 (dieses Profil) |
|---|---|---|---|
| Worklist, Station-AE der Standardabfrage | eigener AE-Titel | **leer** | eigener AE-Titel (Tabelle 33); Abschnitt 4.1.2.5 sagt „leer“ – **Widerspruch im Dokument**, die Bridge verträgt beides |
| Worklist, Modality der Standardabfrage | US | US | **zuletzt benutzter Wert** |
| MPPS | nein | ja (Statuscodes 0110/0116) | ja (Statuscodes **B60x/C60x**) |
| Kompression Bilder | JPEG Baseline/Lossless, RLE, JPEG 2000 | dazu MPEG2 (Doppler-Audio) | wie DP-50, **kein MPEG2** |
| Weitere Storage-Klassen | – | SR, PDF, Raw Data, Enhanced US Volume | nur Comprehensive SR (nicht bei Vet) |
| Zeichensatz | `ISO_IR 100`, GB18030 | UTF-8 (`ISO_IR 192`) bei Deutsch | `ISO_IR 100` bei Deutsch (passt zur Bridge) |
| TLS | – | optional | – (keine Sicherheitsprofile) |
| Implementation Class UID | …1.1150… | …1.2162… | …1.1116… |

## Einstellungen am Gerät (Checkliste, gleiche Serverdaten wie beim DP-50)

Für den Docker-Stack gilt die Tabelle in [`docker/README.md`](../../docker/README.md): **Worklist → Bridge (Port 11112, AE `OPENGDT`),
Storage → Orthanc (Port 4242, AE `ORTHANC`)**.

* Im Dialog „DICOM Service“ Worklist- und Storage-Dienst anlegen; je **Default** setzen, dann fragt das Gerät beim Öffnen der Worklist automatisch.
* **Storage:** Compression Mode auf JPEG Lossy (Baseline) oder unkomprimiert (die Bridge wandelt nur diese um; JPEG Lossless, RLE und JPEG 2000 bleiben
  im Archiv). Welche Werte der Compression Mode anbietet, steht nicht im Dokument.
* **Storage Commitment:** „Associated Storage Service“ leer. **MPPS:** keinen Dienst auf die Bridge zeigen lassen.
* Senden: automatisch nach „End Exam“ („Send or printing after End Exam“), per Sendetaste während der Untersuchung oder manuell (Bildauswahl, iStation).
* SR wird nur gesendet, wenn der SR-Schlüssel installiert ist und der Storage-Dienst „Attach SR when Store Images“ oder „Only Store SR“ hat; die Bridge
  archiviert SR, wandelt sie aber nicht um.

## Profil aktualisieren

Siehe `capture.update_procedure` im JSON und [`../README.md`](../README.md#quellen-überwachen) (`devices/check_sources.py check`).
