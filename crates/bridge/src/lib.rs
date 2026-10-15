//! OpenGDT-Bridge als Bibliothek (die Binary `opengdt-bridge` ist nur die CLI darum).

pub mod config;
pub mod handler;
pub mod http;
pub mod log;
pub mod orthanc;
pub mod pipeline;
pub mod watcher;

use config::Config;
use handler::BridgeHandler;
use opengdt_dicom::scp::{Server, ServerConfig};
use opengdt_store::Archive;
use opengdt_worklist::WorklistDir;
use pipeline::Pipeline;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

pub struct Running {
    pub dicom_addr: SocketAddr,
    pub pipeline: Arc<Pipeline>,
    pub worklist: Arc<WorklistDir>,
    stop: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
}

impl Running {
    /// Beendet alle Threads (nach einem letzten Flush aller offenen Untersuchungen).
    pub fn shutdown(self) {
        self.stop.store(true, Ordering::Relaxed);
        for t in self.threads {
            let _ = t.join();
        }
        self.pipeline.flush(true);
    }

    /// Blockiert, solange der Server läuft (bis `shutdown`/Prozessende).
    pub fn wait(self) {
        for t in self.threads {
            let _ = t.join();
        }
    }
}

/// Startet DICOM-Server, GDT-Watcher und Pipeline-Flusher.
pub fn start(cfg: Config) -> Result<Running, String> {
    let cfg = Arc::new(cfg);
    let worklist =
        Arc::new(WorklistDir::open(&cfg.worklist_dir).map_err(|e| format!("worklist_dir: {e}"))?);
    let archive =
        Arc::new(Archive::open(&cfg.archive_dir).map_err(|e| format!("archive_dir: {e}"))?);
    std::fs::create_dir_all(&cfg.export_dir).map_err(|e| format!("export_dir: {e}"))?;
    std::fs::create_dir_all(&cfg.gdt_export_dir).map_err(|e| format!("gdt_export_dir: {e}"))?;
    let pipeline = Arc::new(Pipeline::new(cfg.clone()));
    let recovered = pipeline.recover(&cfg.archive_dir);
    if recovered > 0 {
        info!("{recovered} noch nicht zurückgemeldete Untersuchungen aus dem Archiv wieder aufgenommen");
    }
    let handler = Arc::new(BridgeHandler {
        cfg: cfg.clone(),
        worklist: worklist.clone(),
        archive: archive.clone(),
        pipeline: pipeline.clone(),
    });
    let server = Server::bind(
        &cfg.listen,
        ServerConfig {
            ae_title: cfg.ae_title.clone(),
            strict_called_ae: cfg.strict_called_ae,
            storage_transfer_syntaxes: cfg.storage_transfer_syntaxes.clone(),
            ..ServerConfig::default()
        },
        handler,
    )
    .map_err(|e| format!("listen {}: {e}", cfg.listen))?;
    let dicom_addr = server.local_addr();
    let stop = server.stop_flag();
    let mut threads = vec![std::thread::spawn(move || server.run())];

    let mut watcher = watcher::Watcher::new(cfg.clone(), worklist.clone())
        .map_err(|e| format!("gdt_import_dir: {e}"))?;
    {
        let stop = stop.clone();
        let wl = worklist.clone();
        let ttl = Duration::from_secs(cfg.worklist_ttl_hours * 3600);
        threads.push(std::thread::spawn(move || {
            let mut ticks = 0u64;
            while !stop.load(Ordering::Relaxed) {
                watcher.scan();
                ticks += 1;
                if ticks.is_multiple_of(600) {
                    match wl.purge_older_than(ttl) {
                        Ok(n) if n > 0 => info!("{n} abgelaufene Worklist-Einträge entfernt"),
                        Ok(_) => {}
                        Err(e) => error!("Worklist-Bereinigung: {e}"),
                    }
                }
                sleep_unless(&stop, Duration::from_secs(1));
            }
        }));
    }
    {
        let stop = stop.clone();
        let p = pipeline.clone();
        threads.push(std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                p.flush(false);
                sleep_unless(&stop, Duration::from_secs(1));
            }
        }));
    }
    if !cfg.orthanc_url.is_empty() {
        let mut poller = orthanc::Poller::new(&cfg, archive, pipeline.clone())?;
        let stop = stop.clone();
        let every = Duration::from_secs(cfg.orthanc_poll_seconds);
        threads.push(std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                poller.poll_once();
                sleep_unless(&stop, every);
            }
        }));
        info!("Orthanc-Anbindung aktiv: {}", cfg.orthanc_url);
    }
    info!("DICOM-Server '{}' lauscht auf {dicom_addr}", cfg.ae_title);
    Ok(Running {
        dicom_addr,
        pipeline,
        worklist,
        stop,
        threads,
    })
}

fn sleep_unless(stop: &AtomicBool, total: Duration) {
    let step = Duration::from_millis(100);
    let mut slept = Duration::ZERO;
    while slept < total && !stop.load(Ordering::Relaxed) {
        std::thread::sleep(step);
        slept += step;
    }
}
