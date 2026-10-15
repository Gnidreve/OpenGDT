#!/bin/bash
# Legt die GNUmed-Datenbank gnumed_v22 im Container `gnumed-db` an – nur wenn sie dort noch nicht fertig vorhanden ist.
# Läuft einmal je `docker compose up` und endet danach. Dauert beim ersten Mal mehrere Minuten (das Schema wird von v2 bis v22
# aufgebaut); danach bleibt die Datenbank im Volume `gnumed-db` erhalten.
set -euo pipefail

DB=gnumed_v22
MARK=opengdt-bootstrap-fertig
export PGHOST="${PGHOST:-gnumed-db}" PGUSER=postgres PGPASSWORD="$(cat /secrets/postgres-password)"
DBO_PW="$(cat /secrets/gm-dbo-password)"
SRC=/var/lib/gnumed

for i in $(seq 1 60); do pg_isready -q && break; sleep 1; done
sq() { psql -qAtX -d postgres -c "$1"; }

drop_staging() { for v in $(seq 2 21); do dropdb --if-exists "gnumed_v$v"; done; }

if [ "$(sq "select 1 from pg_database where datname='$DB'")" = 1 ]; then
  if [ "$(sq "select shobj_description(oid,'pg_database') from pg_database where datname='$DB'")" = "$MARK" ]; then
    drop_staging
    echo "GNUmed-Datenbank $DB ist vorhanden – nichts zu tun."
    exit 0
  fi
  echo "GNUmed-Datenbank $DB ist unvollständig (früherer Abbruch) – wird neu aufgebaut."
fi

echo "GNUmed-Datenbank $DB wird aufgebaut (erster Start: einige Minuten, auf dem Pi länger) …"
# Das Skript findet seine Module relativ zu sich selbst und die Konfiguration wird angepasst: Arbeitskopie des ganzen Baums.
WORK=/tmp/gnumed
rm -rf "$WORK"; cp -a "$SRC" "$WORK"; cd "$WORK/server/bootstrap"
# Konfiguration: Server per TCP statt lokalem Socket, Passwörter aus dem Stack statt der Paketvorgaben (gm-dbo/gm-dbo).
sed -i -e "s/^name = *\$/name = $PGHOST/" \
       -e "s/^#password =.*\$/password = $PGPASSWORD/" \
       -e "s/^password = gm-dbo\$/password = $DBO_PW/" bootstrap-*.conf
# Hinweis: Das Paket legt synthetische Testpatienten an; die Update-Skripte setzen sie voraus (ohne sie bricht der Aufbau ab, geprüft).
mkdir -p /tmp/log
if ! ./bootstrap_gm_db_system.py --log-file=/tmp/log/bootstrap.log --conf-file=bootstrap-latest.conf --quiet; then
  echo "FEHLER: Bootstrap fehlgeschlagen. Ende des Logs:" >&2
  grep -E ' (ERROR|CRITICAL) ' /tmp/log/bootstrap.log | tail -15 | cut -c1-300 >&2
  exit 1
fi
drop_staging
sq "comment on database \"$DB\" is '$MARK'"
echo "GNUmed-Datenbank $DB fertig."
