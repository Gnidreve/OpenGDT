#!/usr/bin/env bash
# Prüft den laufenden Stack (docker compose up -d --build): Dienste, Verbindungen, GDT → Worklist.
set -uo pipefail
cd "$(dirname "$0")/.."
ok=0; bad=0
check() { if eval "$2" >/dev/null 2>&1; then echo "  OK    $1"; ok=$((ok+1)); else echo "  FEHLT $1"; bad=$((bad+1)); fi; }

echo "Dienste:"
check "orthanc läuft"                    "[ \"\$(docker compose ps --status running -q orthanc | wc -l)\" = 1 ]"
check "bridge läuft"                     "[ \"\$(docker compose ps --status running -q bridge | wc -l)\" = 1 ]"
echo "Orthanc:"
PORT=80  # Weboberfläche liegt fest auf 127.0.0.1:80
PW="$(docker compose run --rm --no-deps -T --entrypoint cat init /config/admin-password 2>/dev/null)"
check "Zugangsdaten vorhanden"           "[ -n \"\$PW\" ]"
check "REST ohne Anmeldung wird abgewiesen (401)" "[ \"\$(curl -s -o /dev/null -w '%{http_code}' http://localhost:\$PORT/system)\" = 401 ]"
check "REST mit falschem Passwort abgewiesen"     "[ \"\$(curl -s -o /dev/null -w '%{http_code}' -u praxis:falsch http://localhost:\$PORT/system)\" = 401 ]"
check "REST mit Anmeldung erreichbar (http://localhost)" "curl -sf -u praxis:\"\$PW\" http://localhost:\$PORT/system"
check "Worklist-Plugin geladen"          "curl -sf -u praxis:\"\$PW\" http://localhost:\$PORT/plugins | grep -q -i worklists"
check "Weboberfläche nur auf 127.0.0.1"  "docker compose port orthanc 8042 | grep -q '^127.0.0.1:'"
check "Bridge hat sich bei Orthanc angemeldet" "sleep 3; docker compose logs bridge 2>&1 | grep -q 'Orthanc-Anbindung aktiv' && ! docker compose logs bridge 2>&1 | grep -qE 'verweigert|nicht erreichbar'"
echo "DICOM:"
check "C-ECHO Bridge → Orthanc :4242"    "docker compose exec -T bridge opengdt-bridge echo orthanc:4242 ORTHANC"
check "C-ECHO Bridge → sich selbst"      "docker compose exec -T bridge opengdt-bridge echo 127.0.0.1:11112"
echo "GDT → Worklist:"
rm -f data/worklists/*.wl
docker/send-test-patient.sh >/dev/null
for i in $(seq 1 15); do ls data/worklists/*.wl >/dev/null 2>&1 && break; sleep 1; done
check "Worklist-Datei entstanden"        "ls data/worklists/*.wl"
check "GDT-Eingang aufgeräumt"           "[ ! -e data/gdt-in/PATIN.GDT ]"
echo
echo "Ergebnis: $ok ok, $bad fehlgeschlagen"
[ "$bad" = 0 ]
