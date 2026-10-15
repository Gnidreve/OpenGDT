#!/usr/bin/env bash
# Zeigt noch einmal alle Zugangs- und Verbindungsdaten (dieselbe Ausgabe wie beim Start): GNUmed, Orthanc, Ultraschallsystem.
set -euo pipefail
cd "$(dirname "$0")/.."
docker compose run --rm --no-deps -T -e WAIT=0 info 2>/dev/null
