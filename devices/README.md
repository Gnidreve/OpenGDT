# devices/

Gerätespezifisches Wissen: was ein bestimmtes Ultraschallgerät laut Hersteller tatsächlich
spricht, als maschinenlesbare Daten plus Zusammenfassung. Eine Unterordner je Gerätefamilie.

Jedes Profil trägt Quelle, Zeitstempel und Prüfsumme in seinem `meta`-Block; die Tests lesen es
(siehe [`crates/profile`](../crates/profile/README.md)).

| Ordner | Gerät | Quelle |
|---|---|---|
| [`mindray-dp/`](mindray-dp/README.md) | Mindray DP-70 / DP-50 (und T-/Vet-Varianten) | DICOM Conformance Statement V1.0 (2011) |
