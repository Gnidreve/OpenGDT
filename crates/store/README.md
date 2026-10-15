# opengdt-store

Archiv für empfangene DICOM-Instanzen.

* `Archive::save(&StoredObject)` legt die Instanz **unverändert** als Part-10-Datei unter
  `<root>/<PatientID>/<StudyInstanceUID>/<SOPInstanceUID>.dcm` ab (atomar) und liefert
  `Saved { path, meta }`.
* `load(path)` liest eine archivierte Instanz zurück (Metadaten + Datensatz).
* `InstanceMeta` bündelt die Attribute, die die Pipeline braucht (Patient, Studie,
  Accession, Datum/Zeit, Instanznummer, Modality).
* `safe_component` macht aus DICOM-Werten sichere Dateinamen (verhindert `../`-Pfadtricks).

Das Archiv ist die Quelle der Wahrheit: Konvertierung und GDT-Rückmeldung lesen von hier
und lassen sich bei Fehlern wiederholen.
