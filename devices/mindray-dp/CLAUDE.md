# CLAUDE.md – devices/mindray-dp

* `dicom-conformance.json` ist eine Abschrift des PDFs `046-001907-00 V1.0` (73 Seiten, 13.01.2011).
  Das PDF liegt **nicht** im Repository (Urheberrecht des Herstellers); die Seitenangaben der README
  beziehen sich auf dessen Nummerierung. Neu erzeugt wurde es per Skript aus dem gelesenen PDF –
  bei Korrekturen die JSON-Datei direkt ändern und gültig halten (`python3 -m json.tool`).
* Bekannte Unschärfen im Quelldokument (nicht „korrigieren“, sondern so lassen und annotieren):
  VR von (0010,21B0) als `US` (Standard: `LT`); Tabelle 1 nennt SCP=Yes, Abschnitt 5 nur SCU;
  unklar, ob die Transfer Syntaxes in einem oder mehreren Presentation Contexts vorgeschlagen werden.
* Die Abfrage in `crates/bridge/tests/mindray_dp.rs` ist aus Tabelle 9 nachgebaut. Ändert sich das JSON,
  den Test abgleichen. Der Test hat reale Fehler gefunden (Return-Key-Sequenzen, Implicit-VR-Sequenzen).
* Pilotgerät des Nutzers: **DP-50**, nur „theoretisch zum Testen“ verfügbar; Firmware unbekannt.
