# CLAUDE.md – crates/worklist

* Alle Textwerte im Eintrag sind Latin-1-Bytes; `Dataset::get_str` liest UTF-8, fällt sonst auf
  Latin-1 zurück. Wird hier die Kodierung geändert, `SpecificCharacterSet` mitändern.
* VR `SH` ≤ 16 Zeichen: Accession/StudyID/RequestedProcedureID/SPS-ID teilen sich denselben
  Wert (`entry.rs`, Test `short_identifiers_respect_sh_limit`).
* Matching: ein gespeicherter *leerer* Wert trifft einen *nicht-leeren* Schlüssel nicht
  (DICOM-konform). Deshalb entfernt `bridge/handler.rs` die Stations-Schlüssel aus der
  Abfrage, wenn `station_ae` nicht konfiguriert ist.
* Sequenzen, deren Items nur leere Schlüssel enthalten, sind **Return Keys** und treffen immer – auch
  wenn der Eintrag die Sequenz nicht hat (`is_universal`). Reale Geräte (Mindray DP) senden sie so.
  Echte Suchwerte in einer fehlenden Sequenz treffen nicht.
* `wildcard` ist iterativ (O(Muster × Wert)). Die frühere rekursive Fassung brauchte bei Mustern wie `*a*a*a*b` exponentiell
  lange (eine einzige Netzwerkabfrage hätte die Worklist lahmgelegt). Test `matches_reference_implementation_exhaustively`
  vergleicht mit der rekursiven Referenz – bei Änderungen am Matching bleibt er der Maßstab.
* Defekte `.wl`-Dateien werden beim Lesen übersprungen, nicht gemeldet (Test
  `corrupt_files_are_ignored`).
* `.tmp`-Dateien im Verzeichnis (`.<name>.tmp`) sind Zwischenstände und werden ignoriert.
