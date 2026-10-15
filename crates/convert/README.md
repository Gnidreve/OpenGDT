# opengdt-convert

Macht aus DICOM-Instanzen Dateien, die ein PVS öffnen kann.

Grundsatz: **nichts neu kodieren, was bereits in einem Standardformat vorliegt.**

| Eingang | Ausgang |
|---|---|
| Encapsulated **JPEG Baseline** (`1.2.840.10008.1.2.4.50`) | `.jpg` – die Fragmente werden unverändert ausgepackt |
| Encapsulated **MPEG-4** (`…4.102`–`…4.106`) | `.mp4` – Fragmente konkateniert |
| **Cine-Clip** (Mehrbild) aus JPEG-Baseline-Frames | `.avi` (**MJPEG**) – die JPEGs werden unverändert eingepackt, nichts neu kodiert |
| **Cine-Clip** unkomprimiert | `.avi` unkomprimiert: Graustufen 8 Bit mit Graupalette, Farbe 24 Bit BGR |
| Unkomprimiert, 8/16 Bit Graustufen (MONOCHROME1/2), RGB, YBR_FULL | `.png` (eigener Encoder, unkomprimierte Deflate-Blöcke) |
| Encapsulated PDF (`…104.1`) | `.pdf` |
| alles andere (JPEG 2000, JPEG-LS, RLE, Structured Reports …) | `Error::Unsupported` – das Original bleibt im Archiv |

```rust
let outs = opengdt_convert::convert(sop_class, transfer_syntax, &dataset_bytes, Options::default())?;
```

## Mehrbild-Instanzen (`Options::multi_frame`)

| Wert | Ergebnis |
|---|---|
| `Video` (Standard) | ein AVI (siehe oben); ein einzelnes Bild bleibt ein JPEG/PNG |
| `FirstFrame` | nur der erste Frame als Einzelbild |
| `AllFrames` | jeder Frame als eigene Datei |

**Bildrate:** Frame Time (0018,1063, ms; bei einem Vektor der erste Wert), sonst Cine Rate (0018,0040) oder
Recommended Display Frame Rate (0008,2144), sonst 25 fps. Unsinnige Werte (0, negativ, > 1 s) → 25 fps.
Eine ungleichmäßige Frame Time Vector wird **nicht** abgebildet (konstante Rate).

16-Bit-Graustufen werden in Clips über ein Fenster **aller** Frames auf 8 Bit skaliert (kein Flackern).

## Grenzen

* PNG ist groß (keine Kompression) und für ein PVS evtl. weniger gängig als JPEG –
  ein JPEG-Encoder ist ein möglicher nächster Schritt.
* AVI ist das klassische Format (RIFF, ein Videostream) und auf **2 GiB** begrenzt; größere Clips ergeben
  `Error::Unsupported`. Unkomprimierte Clips werden schnell groß (800×600 Farbe ≈ 1,4 MB je Bild).
* Cine mit JPEG Lossless/RLE/JPEG 2000 wird nicht umgewandelt (Original bleibt im Archiv).
* AVI-Player: MJPG und unkomprimiertes AVI spielen VLC, ffmpeg, Windows Media Player. Ob das **PVS** AVI
  anzeigen kann, ist offen (Frage an den PVS-Hersteller).
