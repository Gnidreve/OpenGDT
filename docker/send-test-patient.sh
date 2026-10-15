#!/usr/bin/env bash
# Legt die Test-Anforderung (Patient 4711, Müller, Jürgen) in den GDT-Eingangsordner – so, als hätte das PVS sie geschrieben.
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p data/gdt-in
cp docker/testdata/PATIN.GDT data/gdt-in/PATIN.GDT
echo "PATIN.GDT liegt in data/gdt-in – in wenigen Sekunden erscheint der Worklist-Eintrag in data/worklists."
