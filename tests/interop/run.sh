#!/usr/bin/env bash
# Startet die Bridge in einem Wegwerf-Verzeichnis und prüft sie mit pynetdicom.
# Benötigt: cargo, python3 mit venv + Internetzugang für `pip` (nur für diesen Test).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
WORK="$(mktemp -d)"
trap 'kill "${BRIDGE_PID:-0}" 2>/dev/null || true; rm -rf "$WORK"' EXIT

cargo build --manifest-path "$ROOT/Cargo.toml" -p opengdt-bridge
python3 -m venv "$WORK/venv"
"$WORK/venv/bin/pip" install -q pynetdicom pillow numpy av

mkdir -p "$WORK/gdt-in"
cat > "$WORK/opengdt.conf" <<CONF
ae_title = OPENGDT
listen = 127.0.0.1:11112
gdt_import_dir = $WORK/gdt-in
gdt_export_dir = $WORK/gdt-out
worklist_dir = $WORK/wl
archive_dir = $WORK/arch
export_dir = $WORK/export
settle_seconds = 2
station_ae = MINDRAY
CONF

"$ROOT/target/debug/opengdt-bridge" run "$WORK/opengdt.conf" > "$WORK/bridge.log" 2>&1 &
BRIDGE_PID=$!
sleep 1

python3 - "$WORK/gdt-in/PATIN.GDT" <<'PY'
import sys
b = b""
for i, v in [(8000, "6301"), (9206, "3"), (3000, "4711"), (3101, "Müller"),
             (3102, "Jürgen"), (3103, "07031965"), (3110, "1")]:
    e = v.encode("latin-1")
    b += ("%03d%04d" % (9 + len(e), i)).encode() + e + b"\r\n"
open(sys.argv[1], "wb").write(b)
PY
sleep 3.5   # der Watcher wartet auf eine stabile Datei

"$WORK/venv/bin/python" "$ROOT/tests/interop/interop.py" "$WORK" || { echo "--- Bridge-Log ---"; cat "$WORK/bridge.log"; exit 1; }
