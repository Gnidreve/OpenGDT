//! Überwacht das GDT-Importverzeichnis des PVS und erzeugt Worklist-Einträge.
//!
//! Kein inotify o. Ä. (std-only): Das Verzeichnis wird gepollt. Eine Datei gilt
//! als fertig geschrieben, wenn Größe und Änderungszeit in zwei aufeinander
//! folgenden Durchläufen identisch waren.

use crate::config::Config;
use opengdt_gdt::{Patient, Record};
use opengdt_worklist::{build_entry, EntryOptions, WorklistDir};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

pub struct Watcher {
    cfg: Arc<Config>,
    worklist: Arc<WorklistDir>,
    seen: HashMap<PathBuf, (u64, Option<SystemTime>)>,
}

impl Watcher {
    pub fn new(cfg: Arc<Config>, worklist: Arc<WorklistDir>) -> std::io::Result<Watcher> {
        fs::create_dir_all(&cfg.gdt_import_dir)?;
        Ok(Watcher {
            cfg,
            worklist,
            seen: HashMap::new(),
        })
    }

    fn is_gdt(p: &Path) -> bool {
        p.is_file()
            && p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("gdt"))
    }

    /// Ein Durchlauf. Gibt die Zahl neu angelegter Worklist-Einträge zurück.
    pub fn scan(&mut self) -> usize {
        let Ok(dir) = fs::read_dir(&self.cfg.gdt_import_dir) else {
            crate::error!(
                "GDT-Importverzeichnis {} nicht lesbar",
                self.cfg.gdt_import_dir.display()
            );
            return 0;
        };
        let mut present = Vec::new();
        let mut created = 0;
        for e in dir.flatten() {
            let path = e.path();
            if !Self::is_gdt(&path) {
                continue;
            }
            let Ok(md) = fs::metadata(&path) else {
                continue;
            };
            let sig = (md.len(), md.modified().ok());
            present.push(path.clone());
            if self.seen.get(&path) == Some(&sig) {
                if self.process(&path) {
                    created += 1;
                }
                self.seen.remove(&path);
            } else {
                self.seen.insert(path, sig);
            }
        }
        self.seen.retain(|p, _| present.contains(p));
        created
    }

    fn process(&self, path: &Path) -> bool {
        match self.try_process(path) {
            Ok(summary) => {
                crate::info!("GDT {} → Worklist: {summary}", path.display());
                self.finish_ok(path);
                true
            }
            Err(msg) => {
                crate::error!("GDT {} abgelehnt: {msg}", path.display());
                let mut bad = path.as_os_str().to_owned();
                bad.push(".err");
                if let Err(e) = fs::rename(path, PathBuf::from(bad)) {
                    crate::error!("konnte {} nicht in .err umbenennen: {e}", path.display());
                }
                false
            }
        }
    }

    fn try_process(&self, path: &Path) -> Result<String, String> {
        let bytes = fs::read(path).map_err(|e| e.to_string())?;
        let rec = Record::parse(&bytes, self.cfg.gdt_strict).map_err(|e| e.to_string())?;
        if !self.cfg.gdt_request_type.is_empty() {
            let got = rec
                .raw(opengdt_gdt::ids::RECORD_TYPE)
                .map(|v| String::from_utf8_lossy(v).to_string());
            if got.as_deref() != Some(self.cfg.gdt_request_type.as_str()) {
                return Err(format!(
                    "Satzart {} statt erwarteter {}",
                    got.as_deref().unwrap_or("(fehlt)"),
                    self.cfg.gdt_request_type
                ));
            }
        }
        let patient = Patient::from_record(&rec).map_err(|e| e.to_string())?;
        let entry = build_entry(
            &patient,
            &EntryOptions {
                modality: self.cfg.modality.clone(),
                station_ae: self.cfg.station_ae.clone(),
                station_name: self.cfg.station_name.clone(),
                procedure_description: self.cfg.procedure_description.clone(),
                utc_offset_minutes: self.cfg.utc_offset_minutes,
            },
        );
        self.worklist
            .add(&entry)
            .map_err(|e| format!("Worklist nicht schreibbar: {e}"))?;
        Ok(format!(
            "Patient {} ({}, {})",
            patient.id, patient.last_name, patient.first_name
        ))
    }

    fn finish_ok(&self, path: &Path) {
        let result = if self.cfg.gdt_keep_processed {
            let dir = self.cfg.gdt_import_dir.join("processed");
            fs::create_dir_all(&dir)
                .and_then(|_| fs::rename(path, dir.join(path.file_name().unwrap_or_default())))
        } else {
            fs::remove_file(path)
        };
        if let Err(e) = result {
            // Kritisch: bliebe die Datei liegen, würde sie endlos neu verarbeitet.
            crate::error!(
                "GDT-Datei {} lässt sich nicht entfernen: {e}",
                path.display()
            );
        }
    }
}
