# Einmaliger Bootstrap der GNUmed-Datenbank (Schema v22) gegen den Container `gnumed-db`.
# Ubuntu 24.04: dort liegt gnumed-server 22.28 (DB v22, passt zum GNUmed-Client 1.8.x); amd64 + arm64.
# Das Paket wird nur entpackt (nicht installiert): seine Abhängigkeit `postgresql` würde einen zweiten Datenbankserver mitbringen.
ARG BASE_IMAGE=ubuntu:24.04
FROM ${BASE_IMAGE}
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates python3 python3-psycopg2 postgresql-client \
 && cd /tmp && apt-get download gnumed-server \
 && dpkg-deb -x gnumed-server_*.deb /tmp/gm \
 && mv /tmp/gm/var/lib/gnumed /var/lib/gnumed \
 && ln -s server /var/lib/gnumed/Gnumed \
 && grep -q '^VER="22"' /var/lib/gnumed/server/bootstrap/bootstrap-latest.sh \
 && rm -rf /tmp/gm /tmp/*.deb /var/lib/apt/lists/*
COPY bootstrap.sh /bootstrap.sh
ENTRYPOINT ["bash", "/bootstrap.sh"]
