//! Dateibasierte Worklist-Ablage: ein Eintrag = eine `.wl`-Datei (DICOM Part 10).

use crate::matching::{matches, project, MatchOptions};
use opengdt_dicom::file::{read_part10, write_part10};
use opengdt_dicom::{t, uid, Dataset};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

pub struct WorklistDir {
    dir: PathBuf,
}

/// Dateisystemtaugliche Version eines Strings.
fn safe(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

impl WorklistDir {
    pub fn open(dir: impl Into<PathBuf>) -> io::Result<WorklistDir> {
        let dir = dir.into();
        fs::create_dir_all(&dir)?;
        Ok(WorklistDir { dir })
    }

    pub fn path(&self) -> &Path {
        &self.dir
    }

    fn files(&self) -> io::Result<Vec<PathBuf>> {
        let mut v = Vec::new();
        for e in fs::read_dir(&self.dir)? {
            let p = e?.path();
            if p.extension().and_then(|x| x.to_str()) == Some("wl") {
                v.push(p);
            }
        }
        v.sort();
        Ok(v)
    }

    fn load(path: &Path) -> Option<Dataset> {
        let p = read_part10(&fs::read(path).ok()?).ok()?;
        Dataset::decode(&p.data, true).ok()
    }

    /// Legt einen Eintrag an. Bestehende Einträge desselben Patienten (PatientID)
    /// werden ersetzt, damit die Liste nicht bei jeder Anforderung wächst.
    pub fn add(&self, entry: &Dataset) -> io::Result<PathBuf> {
        let pid = entry.get_str(t::PATIENT_ID).unwrap_or_default();
        let acc = entry
            .get_str(t::ACCESSION_NUMBER)
            .unwrap_or_else(uid::generate);
        for f in self.files()? {
            if Self::load(&f)
                .and_then(|d| d.get_str(t::PATIENT_ID))
                .as_deref()
                == Some(pid.as_str())
            {
                let _ = fs::remove_file(f);
            }
        }
        let file = write_part10(
            uid::MWL_FIND,
            &uid::generate(),
            uid::EXPLICIT_VR_LE,
            &entry.encode(true),
        );
        let final_path = self.dir.join(format!("{}.wl", safe(&acc)));
        // Erst unter temporärem Namen schreiben, dann umbenennen – Leser sehen nie halbe Dateien.
        let tmp = self.dir.join(format!(".{}.tmp", safe(&acc)));
        fs::write(&tmp, file)?;
        fs::rename(&tmp, &final_path)?;
        Ok(final_path)
    }

    /// Antworten auf eine Worklist-Abfrage.
    pub fn query(&self, query: &Dataset, o: MatchOptions) -> io::Result<Vec<Dataset>> {
        Ok(self
            .files()?
            .iter()
            .filter_map(|f| Self::load(f))
            .filter(|e| matches(e, query, o))
            .map(|e| project(&e, query))
            .collect())
    }

    /// Löscht Einträge, die älter als `max_age` sind. Gibt die Anzahl zurück.
    pub fn purge_older_than(&self, max_age: Duration) -> io::Result<usize> {
        let mut n = 0;
        for f in self.files()? {
            let age = fs::metadata(&f)?
                .modified()
                .ok()
                .and_then(|m| SystemTime::now().duration_since(m).ok())
                .unwrap_or_default();
            if age > max_age && fs::remove_file(&f).is_ok() {
                n += 1;
            }
        }
        Ok(n)
    }

    /// Entfernt den Eintrag zu einer Accession Number (Untersuchung abgeschlossen).
    pub fn remove_by_accession(&self, accession: &str) -> io::Result<bool> {
        let p = self.dir.join(format!("{}.wl", safe(accession)));
        match fs::remove_file(p) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub fn len(&self) -> io::Result<usize> {
        Ok(self.files()?.len())
    }

    pub fn is_empty(&self) -> io::Result<bool> {
        Ok(self.len()? == 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{build_entry, EntryOptions};
    use opengdt_gdt::{Patient, Sex};

    fn tmpdir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("opengdt-test-{name}-{}", uid::generate()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    fn patient(id: &str, last: &str) -> Patient {
        Patient {
            id: id.into(),
            last_name: last.into(),
            first_name: "X".into(),
            birth_date: None,
            sex: Sex::Unknown,
        }
    }

    const O: MatchOptions = MatchOptions { match_dates: false };

    #[test]
    fn add_query_replace_remove() {
        let d = tmpdir("wl");
        let wl = WorklistDir::open(&d).unwrap();
        let o = EntryOptions::default();
        wl.add(&build_entry(&patient("1", "Alpha"), &o)).unwrap();
        wl.add(&build_entry(&patient("2", "Beta"), &o)).unwrap();
        assert_eq!(wl.len().unwrap(), 2);
        // gleicher Patient → ersetzt
        let p = wl.add(&build_entry(&patient("1", "Alpha"), &o)).unwrap();
        assert_eq!(wl.len().unwrap(), 2);

        let mut q = Dataset::new();
        q.set_str(t::PATIENT_NAME, "Be*");
        q.set_str(t::PATIENT_ID, "");
        let r = wl.query(&q, O).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].get_str(t::PATIENT_ID).as_deref(), Some("2"));

        let acc = p.file_stem().unwrap().to_str().unwrap().to_string();
        assert!(wl.remove_by_accession(&acc).unwrap());
        assert_eq!(wl.len().unwrap(), 1);
        assert_eq!(wl.purge_older_than(Duration::from_secs(3600)).unwrap(), 0);
        assert_eq!(wl.purge_older_than(Duration::ZERO).unwrap(), 1);
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn corrupt_files_are_ignored() {
        let d = tmpdir("wlbad");
        let wl = WorklistDir::open(&d).unwrap();
        fs::write(d.join("kaputt.wl"), b"nope").unwrap();
        wl.add(&build_entry(&patient("1", "A"), &EntryOptions::default()))
            .unwrap();
        assert_eq!(wl.query(&Dataset::new(), O).unwrap().len(), 1);
        let _ = fs::remove_dir_all(d);
    }
}
