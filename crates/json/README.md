# opengdt-json

Minimaler JSON-Parser (RFC 8259), nur `std`. Fehlermeldungen mit Zeile/Spalte, Tiefenlimit 64, doppelte
Schlüssel sind ein Fehler, die Schlüsselreihenfolge bleibt erhalten.

```rust
let j = opengdt_json::Json::parse(text)?;
j.at("meta.source.url").and_then(opengdt_json::Json::as_str);
```

Benutzt von [`profile`](../profile/README.md) (Geräteprofile) und [`bridge`](../bridge/README.md) (Orthanc-REST-Antworten).
Nur Lesen, kein Serialisieren.
