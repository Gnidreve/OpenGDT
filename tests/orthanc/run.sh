#!/usr/bin/env bash
# Ende-zu-Ende mit einem ECHTEN Orthanc: pynetdicom (als Gerät) → Orthanc → Bridge (Orthanc-Modus) → GDT/Bilder.
# Benötigt: cargo, python3 (venv + pip), ein installiertes Orthanc (`Orthanc` im PATH, z. B. apt install orthanc).
# Orthanc wird mit docker/orthanc/orthanc.json gestartet (nur Pfade ersetzt) – es wird also die ausgelieferte
# Konfiguration getestet, nicht eine Testvariante.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
command -v Orthanc >/dev/null || { echo "Orthanc nicht gefunden – Test übersprungen (apt install orthanc)"; exit 0; }
WORK="$(mktemp -d)"
trap 'kill ${ORTHANC_PID:-0} ${BRIDGE_PID:-0} 2>/dev/null || true; wait 2>/dev/null || true; rm -rf "$WORK"' EXIT

cargo build --manifest-path "$ROOT/Cargo.toml" -p opengdt-bridge
python3 -m venv "$WORK/venv"
"$WORK/venv/bin/pip" install -q pynetdicom pillow numpy av

mkdir -p "$WORK/worklists" "$WORK/orthanc-db" "$WORK/gdt-in" "$WORK/cfg"
# Pfade ersetzen: Datenbank/Worklists ins Wegwerf-Verzeichnis; das Plugin liegt im Debian-Paket woanders als im Docker-Image.
sed -e "s#/var/lib/orthanc/db#$WORK/orthanc-db#g" -e "s#\"/worklists\"#\"$WORK/worklists\"#" \
    -e "s#/usr/local/share/orthanc/plugins/#/usr/share/orthanc/plugins/#" \
    "$ROOT/docker/orthanc/orthanc.json" > "$WORK/cfg/orthanc.json"
# Ports im Test frei wählen, damit nichts kollidiert
sed -i -e 's/"HttpPort": 8042/"HttpPort": 18042/' -e 's/"DicomPort": 4242/"DicomPort": 14242/' "$WORK/cfg/orthanc.json"
# Zugangsdaten wie im Docker-Stack: eigene Datei im selben Konfigurationsverzeichnis
printf 'TestPw-Praxis-12345' > "$WORK/admin-password"; printf 'TestPw-Bridge-12345\n' > "$WORK/orthanc-password"
cat > "$WORK/cfg/credentials.json" <<CRED
{ "RegisteredUsers": { "praxis": "TestPw-Praxis-12345", "bridge": "TestPw-Bridge-12345" } }
CRED

Orthanc "$WORK/cfg/" > "$WORK/orthanc.log" 2>&1 &
ORTHANC_PID=$!
CURL_AUTH="-u praxis:TestPw-Praxis-12345"
for i in $(seq 1 40); do curl -sf $CURL_AUTH http://127.0.0.1:18042/system >/dev/null 2>&1 && break; sleep 0.5; done
curl -sf $CURL_AUTH http://127.0.0.1:18042/system >/dev/null || { echo "Orthanc startet nicht:"; tail -30 "$WORK/orthanc.log"; exit 1; }
echo "Orthanc läuft: $(curl -s $CURL_AUTH http://127.0.0.1:18042/system | python3 -c 'import sys,json; d=json.load(sys.stdin); print(d["Version"], "AET", d["DicomAet"])')"
# Zugriffsschutz: ohne/mit falschem Passwort 401, die Bridge-Zugangsdaten gelten ebenfalls
[ "$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:18042/system)" = 401 ] || { echo "FEHLER: REST ohne Anmeldung nicht abgewiesen"; exit 1; }
[ "$(curl -s -o /dev/null -w '%{http_code}' -u praxis:falsch http://127.0.0.1:18042/changes)" = 401 ] || { echo "FEHLER: falsches Passwort akzeptiert"; exit 1; }
[ "$(curl -s -o /dev/null -w '%{http_code}' -u bridge:TestPw-Bridge-12345 http://127.0.0.1:18042/system)" = 200 ] || { echo "FEHLER: Bridge-Benutzer abgewiesen"; exit 1; }
echo "Zugriffsschutz ok (401 ohne/mit falschem Passwort)"

cat > "$WORK/opengdt.conf" <<CONF
listen = 127.0.0.1:11112
gdt_import_dir = $WORK/gdt-in
gdt_export_dir = $WORK/gdt-out
worklist_dir = $WORK/worklists
archive_dir = $WORK/arch
export_dir = $WORK/export
settle_seconds = 1
station_ae = MINDRAY
orthanc_url = http://127.0.0.1:18042
orthanc_user = bridge
orthanc_password_file = $WORK/orthanc-password
orthanc_poll_seconds = 1
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
sleep 3.5   # Watcher: stabile Datei → Worklist-Eintrag im von Orthanc gelesenen Ordner

OPENGDT_DICOM_PORT=14242 OPENGDT_DICOM_AE=ORTHANC \
  "$WORK/venv/bin/python" "$ROOT/tests/interop/interop.py" "$WORK" \
  || { echo "--- Bridge-Log ---"; cat "$WORK/bridge.log"; echo "--- Orthanc-Log ---"; tail -40 "$WORK/orthanc.log"; exit 1; }
# Die komplette Geräteabfrage des DP-50 (aus dem Profil) gegen Orthanc: Worklist-Eintrag aus dem Test oben ist noch da.
OPENGDT_EXTERNAL_MWL=127.0.0.1:14242 OPENGDT_QUERY_DATE="$(date -u +%Y%m%d)" \
  cargo test --manifest-path "$ROOT/Cargo.toml" -p opengdt-bridge --test mindray_dp device_query_against_external -- --nocapture 2>&1 | grep -E "^test |panicked|Schlüssel|Treffer|result:" \
  || true
OPENGDT_EXTERNAL_MWL=127.0.0.1:14242 OPENGDT_QUERY_DATE="$(date -u +%Y%m%d)" \
  cargo test --quiet --manifest-path "$ROOT/Cargo.toml" -p opengdt-bridge --test mindray_dp device_query_against_external >/dev/null \
  || { echo "FEHLER: DP-50-Abfrage gegen Orthanc"; tail -30 "$WORK/orthanc.log"; exit 1; }
echo "ORTHANC-E2E OK"
