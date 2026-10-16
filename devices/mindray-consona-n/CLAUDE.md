# CLAUDE.md – devices/mindray-consona-n

* `dicom-conformance.json` ist eine Abschrift von `046-024940-00 V3.0` (331 Seiten, 13.01.2023). Quelle, Prüfsumme und Zeitstempel stehen
  im Block `meta`; das PDF liegt **nicht** im Repository (Urheberrecht). `review_status` bleibt `unreviewed`, bis ein Mensch geprüft hat.
* Erzeugung: Text mit `pdftotext -layout`, Tabellen der IOD-Module (Tabellen 21–50) per Skript gelesen (Zeilen `(gggg,eeee) VR Typ Name Wert`,
  Fortsetzungszeilen nach Spaltenposition), die Worklist-Schlüssel (Tabelle 76) wurden gegen den Quelltext abgeglichen: gleiche 64 Tags in gleicher
  Reihenfolge und Tiefe. Transfer-Syntax-Listen (Tabelle 12) von Hand abgeschrieben. Nicht „bereinigt“: Namen mit `>` sind Unterattribute.
* Bekannte Unschärfen im Quelldokument (so lassen, nicht korrigieren): VR von (0010,21B0) als `US` (Standard `LT`); Tabelle 5 und Tabelle 1 nennen
  für Storage-Klassen SCP=Yes, die Fußnote schränkt auf Query/Retrieve ein; im Dokument fehlt ein Wert für die maximale Trefferzahl der Worklist;
  die Werte von „Compression Mode“ sind nicht aufgeführt; Tippfehler im Original („Abdomen Ultrasoud Report Report“) sind normalisiert.
* Der Titel der Quelle lautet „…N6 and N9…“, das Dokument gilt laut Titelseite für N6, N7, N8 und N9.
* Die Tests (`crates/bridge/tests/device_profiles.rs`) laufen für dieses Profil automatisch mit. Neu hinzugekommen wegen dieses Geräts:
  `default_query_with_empty_station_ae_finds_the_entry`, `mpps_is_not_supported_and_must_not_be_configured_on_the_device`.
* Ungeprüft/offen: ob die Bridge-Worklist-Antwort (ISO_IR 100) am Gerät Umlaute richtig anzeigt; ob Orthanc MPPS annimmt; was ein echtes Gerät
  tatsächlich an Transfer Syntaxes aushandelt (Gerät sendet laut Dokument die „user configured“ komprimierte Syntax).
