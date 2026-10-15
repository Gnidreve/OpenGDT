#!/bin/sh
# Einmaliger Init-Schritt des Stacks (läuft als root im Container `init`, endet danach):
#  1. Datenordner in ./data anlegen und dem Benutzer PUID:PGID übereignen
#  2. Orthanc-Konfiguration und Zugangsdaten in das Volume `orthanc-config` schreiben
#  3. Das Passwort der Bridge in das Volume `bridge-secrets` legen (nur die Bridge sieht es)
#  4. Passwörter für die GNUmed-Datenbank (Postgres-Administrator, gm-dbo) in das Volume `gnumed-secrets` legen
# Passwörter werden beim ersten Start zufällig erzeugt und bleiben erhalten; per .env lassen sie sich setzen.
set -eu

PUID="${PUID:-1000}"
PGID="${PGID:-1000}"

mkdir -p /data/gdt-in /data/gdt-out /data/export /data/archive /data/worklists
chown -R "$PUID:$PGID" /data

CFG=/config
BRIDGE=/bridge-secrets
mkdir -p "$CFG" "$BRIDGE"
cp /static/orthanc.json "$CFG/orthanc.json"

valid() { printf '%s' "$1" | grep -Eq '^[A-Za-z0-9._-]{12,64}$'; }

generate() {
  p=""
  while [ "${#p}" -lt 24 ]; do
    p="$p$(head -c 64 /dev/urandom | base64 | tr -dc 'A-Za-z0-9')"
  done
  printf '%s' "$p" | head -c 24
}

# ensure <Name> <Datei> <Vorgabe aus .env>
ensure() {
  if [ -n "$3" ]; then
    if ! valid "$3"; then
      echo "FEHLER: Passwort für $1 (aus .env) muss 12-64 Zeichen lang sein und darf nur A-Z a-z 0-9 . _ - enthalten." >&2
      exit 1
    fi
    printf '%s' "$3" > "$2"
  elif [ ! -s "$2" ]; then
    generate > "$2"
    echo "Neues Passwort für Benutzer '$1' erzeugt."
  fi
}

ensure praxis "$CFG/admin-password" "${ORTHANC_ADMIN_PASSWORD:-}"
ensure bridge "$CFG/bridge-password" "${ORTHANC_BRIDGE_PASSWORD:-}"

# Orthanc liest alle *.json im Verzeichnis; die Passwörter stehen dort im Klartext (Orthanc-Vorgabe) – das Volume ist nur
# für die Container sichtbar, nicht als Ordner im Repository.
printf '{\n  "RegisteredUsers": {\n    "praxis": "%s",\n    "bridge": "%s"\n  }\n}\n' \
  "$(cat "$CFG/admin-password")" "$(cat "$CFG/bridge-password")" > "$CFG/credentials.json"
chmod 0644 "$CFG/orthanc.json" "$CFG/credentials.json"
chmod 0600 "$CFG/admin-password" "$CFG/bridge-password"

cp "$CFG/bridge-password" "$BRIDGE/orthanc-password"
chown "$PUID:$PGID" "$BRIDGE/orthanc-password"
chmod 0400 "$BRIDGE/orthanc-password"

# GNUmed-Datenbank: Passwörter nur alphanumerisch (landen in Konfigurationsdateien und SQL), werden nie überschrieben.
GM=/gnumed-secrets
mkdir -p "$GM"
for f in postgres-password gm-dbo-password; do
  if [ ! -s "$GM/$f" ]; then
    generate > "$GM/$f"
    echo "Neues Passwort für GNUmed-Datenbank ($f) erzeugt."
  fi
  chmod 0644 "$GM/$f"   # Postgres/Bootstrap lesen als eigener Benutzer; das Volume ist nur für die Container sichtbar
done
