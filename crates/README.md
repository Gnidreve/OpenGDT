# crates/

Cargo-Workspace. Jedes Crate liegt in einem eigenen Ordner mit eigenem `README.md`
(Menschen) und `CLAUDE.md` (Agenten). **Keine externen Abhängigkeiten, nur `std`.**

```
gdt ──────────────┐
dicom ──┬─ worklist ├──▶ bridge  (Binary + Orchestrierung)
        ├─ store  ──┤
        └─ convert ─┘
```

| Crate | Zweck |
|---|---|
| [`gdt`](gdt/README.md) | GDT-Dateien lesen und schreiben |
| [`dicom`](dicom/README.md) | Minimaler DICOM-Stack (Datensätze, Netzwerk, ECHO/FIND/STORE) |
| [`worklist`](worklist/README.md) | GDT-Patient → Worklist-Eintrag, Ablage, Abfrage-Matching |
| [`store`](store/README.md) | Archiv für empfangene Instanzen |
| [`convert`](convert/README.md) | DICOM-Pixeldaten → JPEG/MP4/PNG/PDF |
| [`bridge`](bridge/README.md) | Konfiguration, Watcher, Pipeline, CLI |
| [`json`](json/README.md) | Minimaler JSON-Parser (Geräteprofile, Orthanc-REST) |
| [`profile`](profile/README.md) | Geräteprofile (JSON-Abschrift von Herstellerdokumenten), Validierung; nur für Tests |

Befehle (im Repository-Wurzelordner): `cargo build --workspace`, `cargo test --workspace`,
`cargo clippy --workspace --all-targets`.
