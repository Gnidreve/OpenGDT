# docker/gnumed

Bootstrap-Image für die GNUmed-Datenbank. Wird vom Compose-Stack als Dienst `gnumed-bootstrap` gestartet, legt gegen den Dienst
`gnumed-db` (PostgreSQL 15) einmalig die Datenbank `gnumed_v22` an und endet. Ist sie schon fertig, passiert nichts.

Von Hand: `docker compose run --rm --no-deps gnumed-bootstrap`. Zurücksetzen: `docker compose down -v`.
Hintergrund, Zugangsdaten, Grenzen: [`../README.md`](../README.md#gnumed-datenbank-im-stack).
