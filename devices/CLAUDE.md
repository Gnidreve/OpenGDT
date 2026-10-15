# CLAUDE.md – devices/

* Je Gerätefamilie ein Ordner mit `README.md`, `CLAUDE.md` und den Daten (`*.json`).
* Daten sind **Abschriften aus Herstellerdokumenten**, keine Messungen. Im JSON steht die Quelle
  (`source`); bei Abweichung gilt das Dokument. Nie Werte „ergänzen“, die nicht im Dokument stehen.
* Gerätedaten dürfen in Tests als Vorlage dienen (siehe `crates/bridge/tests/mindray_dp.rs`),
  aber **nicht** zur Laufzeit gelesen werden: Die Bridge ist gerätegenerisch und std-only
  (kein JSON-Parser im Projekt).
* Neues Gerät: Ordner anlegen, Tabelle oben ergänzen, Tests nach dem Muster von `mindray_dp.rs`.
