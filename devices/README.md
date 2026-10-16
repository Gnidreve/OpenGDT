# devices/

Gerätespezifisches Wissen: was ein bestimmtes Ultraschallgerät laut Hersteller tatsächlich
spricht, als maschinenlesbare Daten plus Zusammenfassung. Eine Unterordner je Gerätefamilie.

Jedes Profil trägt Quelle, Zeitstempel und Prüfsumme in seinem `meta`-Block; die Tests lesen es
(siehe [`crates/profile`](../crates/profile/README.md)).

| Ordner | Gerät | Quelle |
|---|---|---|
| [`mindray-dp/`](mindray-dp/README.md) | Mindray DP-70 / DP-50 (und T-/Vet-Varianten), **Pilot** | DICOM Conformance Statement V1.0 (2011) |
| [`mindray-consona-n/`](mindray-consona-n/README.md) | Mindray Consona N6 / N7 / N8 / N9 | DICOM Conformance Statement V3.0 (2023) |

## Quellen überwachen

`devices/check_sources.py` (nur Python-Standardbibliothek; `pdfinfo`/`pdftotext` aus poppler sind optional) prüft anhand des `meta`-Blocks
jedes Profils, ob das Quell-PDF noch dasselbe ist:

```sh
devices/check_sources.py check            # Bericht; Exit-Code 1, wenn sich etwas geändert hat oder die Quelle nicht erreichbar ist
devices/check_sources.py check --update   # bei geänderter Quelle: sha256, Größe, Seitenzahl nachziehen
devices/check_sources.py inspect URL...   # Kenndaten neuer PDFs (Titelseite, Seiten, Prüfsumme) für ein neues Profil
```

`--update` setzt zusätzlich `review_status` zurück auf `unreviewed` und trägt `meta.capture.source_changed_at` ein. **`extracted_at` bleibt
unverändert**: Das ist der Zeitpunkt der Abschrift, und die muss nach einer geänderten Quelle erst neu erstellt werden
(`capture.update_procedure`). Das Skript ändert nur diese Stellen im Text, das Format der Datei bleibt erhalten. Tests: `python3 -I devices/test_check_sources.py`.
