#!/usr/bin/env bash
# Prüft den laufenden Stack (docker compose up -d --build): Dienste, Verbindungen, GDT → Worklist.
set -uo pipefail
cd "$(dirname "$0")/.."
ok=0; bad=0
check() { if eval "$2" >/dev/null 2>&1; then echo "  OK    $1"; ok=$((ok+1)); else echo "  FEHLT $1"; bad=$((bad+1)); fi; }

echo "Dienste:"
check "orthanc läuft"                    "[ \"\$(docker compose ps --status running -q orthanc | wc -l)\" = 1 ]"
check "bridge läuft"                     "[ \"\$(docker compose ps --status running -q bridge | wc -l)\" = 1 ]"
check "gnumed-db läuft"                  "[ \"\$(docker compose ps --status running -q gnumed-db | wc -l)\" = 1 ]"
echo "GNUmed-Datenbank:"
check "Bootstrap abgeschlossen (gnumed_v22 fertig)" "docker compose exec -T gnumed-db psql -U postgres -At -c \"select shobj_description(oid,'pg_database') from pg_database where datname='gnumed_v22'\" | grep -q opengdt-bootstrap-fertig"
check "Postgres-Einstellungen für den GNUmed-Client (log_connections, log_disconnections, track_commit_timestamp)" "[ \"\$(docker compose exec -T gnumed-db psql -U postgres -At -c \"select count(*) from pg_settings where (name,setting) in (('log_connections','on'),('log_disconnections','on'),('track_commit_timestamp','on'))\")\" = 3 ]"
check "Postgres nur auf 127.0.0.1:5432"  "docker compose port gnumed-db 5432 | grep -q '^127.0.0.1:5432'"
pgc() { docker compose run --rm --no-deps -T -e PGHOST=gnumed-db -e PGPASSWORD="$1" --entrypoint psql gnumed-bootstrap -h gnumed-db -U "$2" -d gnumed_v22 -At -c 'select 1'; }
check "Anmeldung any-doc über das Stack-Netzwerk" "pgc any-doc any-doc"
check "Paket-Standardpasswort von gm-dbo abgelehnt" "! pgc gm-dbo gm-dbo"
echo "Orthanc:"
PORT=80  # Weboberfläche liegt fest auf 127.0.0.1:80
PW="$(docker compose run --rm --no-deps -T --entrypoint cat init /config/admin-password 2>/dev/null)"
check "Zugangsdaten vorhanden"           "[ -n \"\$PW\" ]"
check "REST ohne Anmeldung wird abgewiesen (401)" "[ \"\$(curl -s -o /dev/null -w '%{http_code}' http://localhost:\$PORT/system)\" = 401 ]"
check "REST mit falschem Passwort abgewiesen"     "[ \"\$(curl -s -o /dev/null -w '%{http_code}' -u praxis:falsch http://localhost:\$PORT/system)\" = 401 ]"
check "REST mit Anmeldung erreichbar (http://localhost)" "curl -sf -u praxis:\"\$PW\" http://localhost:\$PORT/system"
check "Worklist-Plugin geladen"          "curl -sf -u praxis:\"\$PW\" http://localhost:\$PORT/plugins | grep -q -i worklists"
check "Weboberfläche nur auf 127.0.0.1"  "docker compose port orthanc 8042 | grep -q '^127.0.0.1:'"
check "Bridge hat sich bei Orthanc angemeldet" "sleep 3; L=\$(docker compose logs bridge 2>&1); echo \"\$L\" | grep -q 'Orthanc-Anbindung aktiv' && ! echo \"\$L\" | grep -q 'verweigert' && { ! echo \"\$L\" | grep -q 'nicht erreichbar' || echo \"\$L\" | grep -q 'wieder erreichbar'; }"
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
