# CLAUDE.md – devices/mindray-z60

* `dicom-conformance.json` ist eine Abschrift von `046-017363-00 V1.0` (201 Seiten, 05.06.2019). Quelle, Prüfsumme und Zeitstempel stehen im
  Block `meta`; das PDF liegt **nicht** im Repository. `review_status` bleibt `unreviewed`, bis ein Mensch geprüft hat.
* Erzeugung: `pdftotext -layout`; die Storage-Tabelle (Tabelle 14: Modul-Überschriften `Module: … (M)`, Zeilen `(gggg,eeee) VR Typ Name Wert`) per Skript;
  Worklist-Schlüssel (Tabelle 33) maschinell abgeglichen: dieselben 64 Tags in gleicher Reihenfolge und Tiefe wie im Consona-N-Profil (deshalb übernommen,
  nur die Vorgaben der Standardabfrage angepasst). Transfer-Syntax-Listen (Tabelle 11) von Hand abgeschrieben.
* Bekannte Unschärfen im Quelldokument (so lassen): Abschnitt 4.1.2.5 („Scheduled Station AE Title is blank“) widerspricht Tabelle 33 („default set to
  your AE title“); VR von (0010,21B0) als `US`; der SR-Context in Tabelle 11 führt Bildkompressionen auf; der Satz zu Lossy Image Compression in Tabelle 14 ist
  mehrdeutig; MPPS-Statuscodes (B60x/C60x) weichen vom Consona-N-Dokument ab (0110/0116).
* Der Titel nennt viele Modelle in einer Zeile (`meta.source.models`); Z60 Vet/Z6 Vet haben keinen Worklist-/MPPS-/Q-R-/SR-Dienst (Fußnoten Tabelle 1/3).
* Keine eigenen Tests: `crates/bridge/tests/device_profiles.rs` läuft automatisch mit.
