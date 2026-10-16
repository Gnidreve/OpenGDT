#!/usr/bin/env python3
"""Prüft, ob die Quell-PDFs der Geräteprofile (devices/*/dicom-conformance.json) noch unverändert sind.

Nur Python-Standardbibliothek (Dev-Werkzeug außerhalb von Cargo). Optional: `pdfinfo`/`pdftotext` (poppler) für Seitenzahl und Titelseite.

  devices/check_sources.py check            # alle Profile prüfen, nichts ändern (Exit 1 bei Änderung/Fehler)
  devices/check_sources.py check --update   # bei geänderter Quelle die Metadaten nachziehen (siehe unten)
  devices/check_sources.py inspect URL...   # PDF(s) laden und Kenndaten der Titelseite zeigen (für neue Profile)

`--update` ändert bei geänderter Quelle: file_sha256, file_bytes, pages (und nur diese Stellen im Text; das Format der Datei
bleibt erhalten), setzt review_status auf "unreviewed" (reviewed_by/reviewed_at auf null) und trägt meta.capture.source_changed_at ein.
`extracted_at` bleibt bewusst unverändert: Es ist der Zeitpunkt der Abschrift, und die hat niemand wiederholt. Danach muss die
Abschrift neu erstellt werden (meta.capture.update_procedure).
"""
import argparse
import datetime
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import urllib.error
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
# Die Mindray-CDN weist Anfragen ohne Browser-Kennung mit 403 ab.
UA = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36"
OK, CHANGED, FIXED_META, UNREACHABLE, BAD = "OK", "GEÄNDERT", "METADATEN-ABWEICHUNG", "NICHT ERREICHBAR", "KEIN PDF"


def now_utc():
    return datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def fetch(url, timeout=120):
    req = urllib.request.Request(url, headers={"User-Agent": UA, "Accept": "application/pdf,*/*"})
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return r.read()


def pdf_pages(data):
    """Seitenzahl über `pdfinfo`; None, wenn das Werkzeug fehlt oder die Datei nicht lesbar ist."""
    if not shutil.which("pdfinfo"):
        return None
    with tempfile.NamedTemporaryFile(suffix=".pdf") as f:
        f.write(data)
        f.flush()
        out = subprocess.run(["pdfinfo", f.name], capture_output=True, text=True)
    m = re.search(r"^Pages:\s+(\d+)", out.stdout, re.M)
    return int(m.group(1)) if m else None


def profiles(devices_dir):
    out = []
    for name in sorted(os.listdir(devices_dir)):
        p = os.path.join(devices_dir, name, "dicom-conformance.json")
        if os.path.isfile(p):
            out.append(p)
    return out


def sub_once(text, pattern, repl, what):
    new, n = re.subn(pattern, repl, text, count=0, flags=re.M)
    if n != 1:
        raise SystemExit(f"FEHLER: {what}: erwartet genau eine Stelle, gefunden {n}")
    return new


def rewrite(text, sha=None, size=None, pages=None, changed_at=None):
    """Ändert nur die genannten Stellen im Profiltext (kein Neuformatieren)."""
    if sha is not None:
        text = sub_once(text, r'("file_sha256"\s*:\s*")[0-9a-f]{64}(")', rf"\g<1>{sha}\g<2>", "file_sha256")
    if size is not None:
        text = sub_once(text, r'("file_bytes"\s*:\s*)\d+', rf"\g<1>{size}", "file_bytes")
    if pages is not None:
        text = sub_once(text, r'("pages"\s*:\s*)\d+', rf"\g<1>{pages}", "pages")
    if changed_at is not None:
        text = sub_once(text, r'("review_status"\s*:\s*)"[^"]*"', r'\g<1>"unreviewed"', "review_status")
        text = sub_once(text, r'("reviewed_by"\s*:\s*)(null|"[^"]*")', r"\g<1>null", "reviewed_by")
        text = sub_once(text, r'("reviewed_at"\s*:\s*)(null|"[^"]*")', r"\g<1>null", "reviewed_at")
        if re.search(r'"source_changed_at"', text):
            text = sub_once(text, r'("source_changed_at"\s*:\s*)"[^"]*"', rf'\g<1>"{changed_at}"', "source_changed_at")
        else:
            text = sub_once(
                text,
                r'^(\s*)("review_status"\s*:\s*"unreviewed",)$',
                rf'\g<1>\g<2>\n\g<1>"source_changed_at": "{changed_at}",',
                "source_changed_at einfügen",
            )
    return text


def check_one(path, update, fetcher=fetch):
    text = open(path, encoding="utf-8").read()
    meta = json.loads(text)["meta"]
    src = meta["source"]
    pid = meta.get("profile_id", os.path.basename(os.path.dirname(path)))
    try:
        data = fetcher(src["url"])
    except (urllib.error.URLError, OSError, ValueError) as e:
        return pid, UNREACHABLE, f"{src['url']}: {e}"
    if not data.startswith(b"%PDF"):
        return pid, BAD, "Antwort ist kein PDF (Fehlerseite der CDN?)"
    sha, size, pages = hashlib.sha256(data).hexdigest(), len(data), pdf_pages(data)
    if sha != src["file_sha256"]:
        msg = f"Quelle geändert: sha256 {src['file_sha256'][:12]}… → {sha[:12]}…, {src['file_bytes']} → {size} Bytes"
        if pages is not None:
            msg += f", Seiten {src['pages']} → {pages}"
        if update:
            new = rewrite(text, sha, size, pages, now_utc())
            open(path, "w", encoding="utf-8").write(new)
            msg += " – Metadaten aktualisiert, Status auf unreviewed; Abschrift muss neu erstellt werden"
        return pid, CHANGED, msg
    drift = []
    if size != src["file_bytes"]:
        drift.append(f"file_bytes {src['file_bytes']} → {size}")
    if pages is not None and pages != src["pages"]:
        drift.append(f"pages {src['pages']} → {pages}")
    if drift:
        if update:
            open(path, "w", encoding="utf-8").write(rewrite(text, None, size, pages))
        return pid, FIXED_META, "gleiche Datei, aber Metadaten stimmten nicht: " + ", ".join(drift)
    return pid, OK, f"sha256 {sha[:12]}… unverändert"


def cmd_check(args):
    files = profiles(args.devices)
    if not files:
        print("keine Profile gefunden", file=sys.stderr)
        return 2
    bad = 0
    for p in files:
        pid, status, msg = check_one(p, args.update)
        print(f"{status:22} {pid}: {msg}")
        bad += status not in (OK,)
    return 1 if bad else 0


def field(text, pattern):
    m = re.search(pattern, text, re.M)
    return m.group(1).strip() if m else None


def cmd_inspect(args):
    for url in args.urls:
        print(f"== {url}")
        try:
            data = fetch(url)
        except (urllib.error.URLError, OSError) as e:
            print(f"   NICHT ERREICHBAR: {e}")
            continue
        if not data.startswith(b"%PDF"):
            print("   kein PDF")
            continue
        print(f"   Datei:   {url.rsplit('/', 1)[-1]}")
        print(f"   Bytes:   {len(data)}   sha256: {hashlib.sha256(data).hexdigest()}")
        print(f"   Seiten:  {pdf_pages(data)}")
        if shutil.which("pdftotext"):
            with tempfile.NamedTemporaryFile(suffix=".pdf") as f:
                f.write(data)
                f.flush()
                txt = subprocess.run(["pdftotext", "-l", "6", "-layout", f.name, "-"], capture_output=True, text=True).stdout
            head = [l.strip() for l in txt.splitlines() if l.strip()][:14]
            print("   Titelseite: " + " | ".join(head[:9]))
            pn = field(txt, r"P/N[:：\s]*([0-9]{3}-[0-9]{6}-[0-9]{2}[^\s]*)")
            ver = field(txt, r"(?:Version|Ver\.?)\s*:?\s*(V?\d+\.\d+[^\n]{0,24})")
            pn = pn or field(url.rsplit("/", 1)[-1], r"(?:^|[^0-9])(046-[0-9]{6}-[0-9]{2})") or None
            print(f"   P/N:     {pn}")
            print(f"   Version: {ver}")
    return 0


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    c = sub.add_parser("check", help="alle Profile prüfen")
    c.add_argument("--update", action="store_true", help="Metadaten bei geänderter Quelle nachziehen")
    c.add_argument("--devices", default=HERE, help="Ordner mit den Geräteprofilen (Standard: dieser Ordner)")
    c.set_defaults(fn=cmd_check)
    i = sub.add_parser("inspect", help="PDF(s) laden und Kenndaten zeigen")
    i.add_argument("urls", nargs="+")
    i.set_defaults(fn=cmd_inspect)
    args = ap.parse_args(argv)
    return args.fn(args)


if __name__ == "__main__":
    sys.exit(main())
