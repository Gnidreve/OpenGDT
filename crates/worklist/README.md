# opengdt-worklist

Macht aus GDT-Patientendaten **DICOM-Modality-Worklist-Einträge** und beantwortet damit
die Abfragen des Ultraschallgeräts.

* `entry` – `build_entry(&Patient, &EntryOptions)` → Worklist-`Dataset`. Namen werden als
  Latin-1 (`ISO_IR 100`) abgelegt; nicht darstellbare Zeichen werden zu `?`, `^` und `\` in
  Namen entfernt. Accession Number: `G` + `JJMMTThhmmss` + 3-stellige Folgenummer (16 Zeichen,
  passend zur VR `SH`).
* `store` – `WorklistDir`: ein Eintrag = eine `.wl`-Datei (Part 10) in einem Verzeichnis.
  Derselbe Patient (PatientID) ersetzt seinen alten Eintrag. Schreiben atomar (temp + rename).
  Abgelaufene Einträge: `purge_older_than`.
* `matching` – Schlüsselvergleich nach DICOM: Wildcards `*`/`?`, Datumsbereiche,
  Sequenz-Schlüssel (`ScheduledProcedureStepSequence`), leere Schlüssel = „alles“.
  `project` liefert nur die angefragten Attribute zurück.
* `time` – Datum/Uhrzeit (UTC) ohne Abhängigkeiten.

## Bekannte Einschränkung: Datumsfilter

Geräte fragen oft „Einträge für heute“. Ohne Zeitzonen-Datenbank (nur `std`) kennt die
Bridge nur UTC plus festen Versatz (`utc_offset_minutes`), der Sommerzeit nicht abbildet.
Deshalb ist das Datumsmatching **standardmäßig aus** (`match_dates = false`);
Aktualität regelt stattdessen die Lebensdauer der Einträge (`worklist_ttl_hours`).
