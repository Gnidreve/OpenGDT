# opengdt-profile

Liest **Geräteprofile**: maschinenlesbare Abschriften von Herstellerdokumenten (z. B. DICOM
Conformance Statements) mit Quellenangabe und Zeitstempel. Aktuell: [`devices/mindray-dp`](../../devices/mindray-dp/README.md) [`devices/mindray-consona-n`](../../devices/mindray-consona-n/README.md) und [`devices/mindray-z60`](../../devices/mindray-z60/README.md).

* `json` – eigener JSON-Parser (RFC 8259, std-only): Fehlerpositionen, Tiefenlimit, doppelte Schlüssel
  sind Fehler, Schlüsselreihenfolge bleibt erhalten.
* `Profile::parse` / `Profile::load` – parst **und validiert**; liefert alle Verstöße auf einmal.
* Typisierte Sicht: `meta()`, `worklist_keys()` (verschachtelt), `storage_contexts()`, `sop_classes()`,
  `worklist_transfer_syntaxes()`, `find_sop_class(..)`.

## Format `opengdt-device-profile/1`

```jsonc
{
  "meta": {
    "schema": "opengdt-device-profile/1",
    "profile_id": "mindray-dp",
    "source":  { "url": "https://…", "title": "…", "part_number": "…", "document_version": "1.0",
                 "published": "2011-01-13", "file_sha256": "…", "file_bytes": 543745, "pages": 73 },
    "capture": { "extracted_at": "2026-10-15T16:04:51Z",     // UTC, Zeitpunkt der Abschrift
                 "extracted_by": "…", "review_status": "unreviewed" | "reviewed",
                 "reviewed_by": null, "reviewed_at": null, "verified_against_device": false }
  },
  "sop_classes": [ … ], "storage": { "proposed_contexts": [ … ] }, "worklist": { "keys": [ … ] }, …
}
```

Die Validierung prüft u. a.: Schema-Kennung, `https`-URL, SHA-256 (64 Hex-Zeichen), Datums-/Zeitformate,
Review-Pflichtfelder bei `reviewed`, alle UIDs (nur Ziffern/Punkte, ≤ 64 Zeichen, keine führenden Nullen),
Tags (`GGGG,EEEE`, Großbuchstaben), bekannte VRs, keine doppelten Tags je Ebene, `items` nur bei `SQ`.

## Benutzung in Tests

```rust
let p = opengdt_profile::Profile::load(path)?;
for c in p.storage_contexts() { /* gegen die Bridge prüfen */ }
```

Beispiel: [`crates/bridge/tests/device_profiles.rs`](../bridge/tests/device_profiles.rs).

## Grenzen

Nur Lesen, kein Schreiben/Serialisieren. Wird **nicht zur Laufzeit der Bridge** benutzt (Dev-Abhängigkeit
von `bridge`). Die Schemas der Abschnitte außerhalb von `meta`, `sop_classes`, `storage.proposed_contexts`
und `worklist` sind (noch) nicht validiert, nur dokumentiert.
