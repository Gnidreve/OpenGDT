#!/bin/bash
# Gibt nach dem Start alle Zugangs- und Verbindungsdaten aus, getrennt nach GNUmed, Orthanc und Ultraschallsystem.
# Läuft im Container `info` (Host-Netzwerk, damit die IP-Adresse des Pi ermittelt werden kann).
# Auch einzeln abrufbar: docker/show-credentials.sh
set -u

wait_for_port() { # Host Port Sekunden
  local i
  for ((i = 0; i < $3; i++)); do
    (exec 3<>"/dev/tcp/$1/$2") 2>/dev/null && return 0
    sleep 1
  done
  return 1
}

if [ "${WAIT:-1}" = 1 ]; then
  wait_for_port 127.0.0.1 80 90 || echo "Hinweis: Orthanc antwortet noch nicht auf Port 80 – Angaben unten trotzdem gültig."
fi

admin_pw="$(cat /config/admin-password 2>/dev/null || echo '(noch nicht erzeugt – docker compose logs init)')"
host_data="${HOST_DATA_DIR:-./data}"
prefix="${PVS_EXPORT_PREFIX:-$host_data/export}"
ips="$(hostname -I 2>/dev/null | tr ' ' '\n' | grep -E '^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$' | grep -v '^127\.' | grep -v '^172\.\(1[6-9]\|2[0-9]\|3[01]\)\.' | tr '\n' ' ')"
ip1="$(echo "$ips" | awk '{print $1}')"
[ -n "$ip1" ] || ip1="<IP-Adresse des Pi>"

line() { printf '%s\n' "════════════════════════════════════════════════════════════════════════════"; }
thin() { printf '%s\n' "────────────────────────────────────────────────────────────────────────────"; }

echo
line
echo " OpenGDT läuft.  Zugangs- und Verbindungsdaten (drei Bereiche):"
line
echo
echo " 1) GNUmed (PVS)   –   Datenbank läuft in diesem Stack, der GNUmed-Client läuft direkt auf dem Pi"
thin
echo "   Server/Host:   localhost     Port: 5432     (nur auf dem Pi erreichbar)"
echo "   Datenbank:     gnumed_v22    (passt zum GNUmed-Client 1.8.x)"
echo "   Anmeldung im GNUmed-Client:  Benutzer any-doc, Passwort any-doc  (Paketvorgabe; am Datenbankserver geprüft,"
echo "                  im Client ungetestet). Das ist NICHT das Orthanc-Passwort."
echo "   Datenbank-Eigentümer gm-dbo: Passwort $(cat /gnumed-secrets/gm-dbo-password 2>/dev/null || echo '(noch nicht erzeugt)')  (nur für Administration)"
echo "   Enthält synthetische Testpatienten aus dem GNUmed-Paket."
echo "   GDT-Ordner für die Anbindung (auf dem Pi):"
echo "     Anforderung  PVS → Bridge:   $host_data/gdt-in"
echo "     Rückmeldung  Bridge → PVS:   $host_data/gdt-out"
echo "     Bilder (Pfad steht in der Rückmeldung):  $prefix"
case "$prefix" in
  /data/*) echo "     ⚠ Das ist ein Pfad INNERHALB des Containers – GNUmed auf dem Pi findet dort nichts. PVS_EXPORT_PREFIX in .env setzen." ;;
esac
echo "   Satzart der Rückmeldung: ${GDT_RESULT_TYPE:-6310} (ungeprüft, mit GNUmed abstimmen)"
if [ -n "${GDT_RESULT_FILENAME:-}" ]; then
  echo "   Dateiname der Rückmeldung: ${GDT_RESULT_FILENAME}"
else
  echo "   Dateiname der Rückmeldung: je Untersuchung <Accession-Nr>.gdt"
fi
echo
echo " 2) Orthanc   –   Weboberfläche (Bildspeicher)"
thin
echo "   Adresse:   http://localhost      (Port 80, nur im Browser des Pi erreichbar)"
echo "   Benutzer:  praxis"
echo "   Passwort:  $admin_pw"
echo "   (Die Bridge meldet sich mit einem eigenen Benutzer an; dessen Passwort brauchst du nie.)"
echo
echo " 3) Ultraschallsystem (Mindray DP-50)   –   am Gerät unter DICOM-Dienste eintragen"
thin
echo "   IP-Adresse des Pi:  ${ips:-<nicht ermittelbar>}"
echo
echo "   Worklist-Server:    IP $ip1    Port 11112    AE-Titel des Servers: OPENGDT"
echo "   Storage-Server:     IP $ip1    Port 4242     AE-Titel des Servers: ORTHANC"
echo "   Storage Commitment: nicht einrichten (leer lassen)"
echo
echo "   Eigener AE-Titel des Geräts:  frei wählbar, z. B. DP50-1  (max. 16 Zeichen)"
echo "   Benutzername/Passwort am Gerät:  keine – DICOM kennt hier keine Anmeldung."
echo
echo "   Mehrere Geräte: überall dieselben Serverdaten (IP, Ports, AE-Titel des Servers), nur der eigene"
echo "   AE-Titel jedes Geräts unterscheidet sich (z. B. DP50-1, DP50-2). AE-Titel werden nicht geprüft."
if [ -n "${DEVICE_AE:-}" ]; then
  echo "   ⚠ DEVICE_AE=${DEVICE_AE} ist gesetzt: Worklist-Einträge gelten nur für dieses Gerät (AE-Titel exakt so am Gerät)."
else
  echo "   Worklist-Einträge gelten für jedes Gerät (DEVICE_AE ist leer)."
fi
echo
line
