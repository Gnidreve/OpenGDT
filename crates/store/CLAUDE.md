# CLAUDE.md – crates/store

* **Sicherheit:** PatientID und UIDs kommen von außen (Netzwerk). Jeder Pfadbestandteil muss
  durch `safe_component` laufen. Neue Pfade nie direkt aus DICOM-Werten bauen.
* `save` dekodiert den Datensatz einmal (für die Metadaten), hält ihn aber nicht fest.
  Pixeldaten nicht in `Saved` zurückgeben – Cine-Clips können sehr groß sein.
* Bei bereits vorhandener SOP-Instanz wird überschrieben (Geräte senden bei Fehlern erneut).
