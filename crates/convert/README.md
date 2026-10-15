# opengdt-convert

Macht aus DICOM-Instanzen Dateien, die ein PVS öffnen kann.

Grundsatz: **nichts neu kodieren, was bereits in einem Standardformat vorliegt.**

| Eingang | Ausgang |
|---|---|
| Encapsulated **JPEG Baseline** (`1.2.840.10008.1.2.4.50`) | `.jpg` – die Fragmente werden unverändert ausgepackt |
| Encapsulated **MPEG-4** (`…4.102`–`…4.106`) | `.mp4` – Fragmente konkateniert |
| Unkomprimiert, 8/16 Bit Graustufen (MONOCHROME1/2), RGB, YBR_FULL | `.png` (eigener Encoder, unkomprimierte Deflate-Blöcke) |
| Encapsulated PDF (`…104.1`) | `.pdf` |
| alles andere (JPEG 2000, JPEG-LS, RLE, Structured Reports …) | `Error::Unsupported` – das Original bleibt im Archiv |

```rust
let outs = opengdt_convert::convert(sop_class, transfer_syntax, &dataset_bytes, Options::default())?;
```

Mehrbild-Instanzen ohne Videoformat liefern standardmäßig nur den **ersten** Frame
(`Options::all_frames` für alle).

## Grenzen

* PNG ist groß (keine Kompression) und für ein PVS evtl. weniger gängig als JPEG –
  ein JPEG-Encoder ist ein möglicher nächster Schritt.
* Cine-Loops aus Einzel-Frames werden nicht zu einem Video zusammengesetzt.
