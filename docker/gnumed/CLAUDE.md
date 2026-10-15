# CLAUDE.md – docker/gnumed

* Die Entscheidungen und Fallen (Postgres 15, Paket nur entpacken, Arbeitskopie, Testpatienten nicht entfernen, Sandbox-Build) stehen in
  [`../CLAUDE.md`](../CLAUDE.md) im Abschnitt „GNUmed-Datenbank“. Hier nicht duplizieren.
* `Dockerfile` bricht den Build ab, wenn das Paket nicht Schema v22 liefert (`VER="22"` im Skript) – Absicht: Client (1.8.x) und DB müssen passen.
* `bootstrap.sh` muss idempotent bleiben (Marker `opengdt-bootstrap-fertig`); nie Datenbanken löschen, die den Marker tragen.
