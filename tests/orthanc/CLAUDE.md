# CLAUDE.md – tests/orthanc/

* Nutzt `tests/interop/interop.py` (Ziel per Umgebung `OPENGDT_DICOM_PORT`/`OPENGDT_DICOM_AE`, Worklist optional getrennt
  über `OPENGDT_WORKLIST_PORT`/`OPENGDT_WORKLIST_AE`) und den Test `device_query_against_external_worklist_server` in
  `crates/bridge/tests/mindray_dp.rs` (aktiv nur mit `OPENGDT_EXTERNAL_MWL`).
* Ports 14242/18042/11112 sind fest; Orthanc-Binary aus dem Debian-Paket (Plugin-Pfad weicht vom Docker-Image ab, `run.sh` ersetzt ihn).
* Reines Dev-Werkzeug; die Python-Pakete dürfen nie ins Projekt wandern (Projektregel: std-only).
