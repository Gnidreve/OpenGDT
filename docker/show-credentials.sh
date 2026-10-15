#!/usr/bin/env bash
# Zeigt URL und Anmeldedaten der Orthanc-Weboberfläche (Benutzer "praxis").
set -euo pipefail
cd "$(dirname "$0")/.."
pw="$(docker compose run --rm --no-deps -T --entrypoint cat init /config/admin-password 2>/dev/null)"
echo "Orthanc-Weboberfläche (nur im Browser des Pi): http://localhost"
echo "Benutzer: praxis"
echo "Passwort: $pw"
