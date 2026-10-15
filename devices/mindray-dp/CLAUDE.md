# CLAUDE.md – devices/mindray-dp

* `dicom-conformance.json` ist eine Abschrift des PDFs `046-001907-00 V1.0` (73 Seiten, 13.01.2011), Quelle,
  Prüfsumme und Zeitstempel stehen im Block `meta`. Das PDF liegt **nicht** im Repository (Urheberrecht des
  Herstellers). Bei Korrekturen die JSON-Datei direkt ändern; `cargo test -p opengdt-profile` validiert sie.
  `review_status` bleibt `unreviewed`, bis ein Mensch die Abschrift geprüft hat – nicht selbst auf `reviewed` setzen.
* Bekannte Unschärfen im Quelldokument (nicht „korrigieren“, sondern so lassen und annotieren):
  VR von (0010,21B0) als `US` (Standard: `LT`); Tabelle 1 nennt SCP=Yes, Abschnitt 5 nur SCU;
  unklar, ob die Transfer Syntaxes in einem oder mehreren Presentation Contexts vorgeschlagen werden.
* `crates/bridge/tests/mindray_dp.rs` baut seine Abfragen und Erwartungen **aus dem JSON** (Worklist-Schlüssel,
  Transfer Syntaxes, SOP-Klassen). Der Test hat reale Fehler gefunden (Return-Key-Sequenzen, Implicit-VR-Sequenzen).
* Pilotgerät des Nutzers: **DP-50**, nur „theoretisch zum Testen“ verfügbar; Firmware unbekannt.
