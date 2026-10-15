# CLAUDE.md – crates/

* **Nur `std`.** Kein neues `[dependencies]`-Eintrag außer Pfad-Abhängigkeiten auf
  Schwester-Crates. Braucht etwas ein externes Crate, ist das eine Entscheidung des
  Nutzers – Rückfrage statt Einbau.
* Abhängigkeitsrichtung: `bridge → {worklist, store, convert, gdt, dicom}`,
  `worklist/store/convert → dicom`, `worklist → gdt`. `gdt` und `dicom` kennen sich nicht.
  Keine Zyklen, `bridge` ist das einzige Crate mit Orchestrierungslogik.
* Neues Crate: Ordner + `Cargo.toml` + `README.md` + `CLAUDE.md`, Eintrag in der Workspace-
  `Cargo.toml`, in der Tabelle in `crates/README.md` und im Wurzel-`README.md`.
* Vor „fertig“: `cargo fmt --all`, `cargo clippy --workspace --all-targets` (ohne Warnungen),
  `cargo test --workspace`.
* Tests liegen neben dem Code (`#[cfg(test)]`) oder in `tests/` des jeweiligen Crates.
