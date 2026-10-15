//! Verbindet DICOM-Server, Worklist, Archiv und Pipeline.

use crate::config::Config;
use crate::pipeline::Pipeline;
use opengdt_dicom::assoc::{
    STATUS_PROCESSING_FAILURE, STATUS_REFUSED_OUT_OF_RESOURCES, STATUS_SUCCESS,
};
use opengdt_dicom::scp::{Handler, PeerInfo, StoredObject};
use opengdt_dicom::{t, Dataset};
use opengdt_store::Archive;
use opengdt_worklist::{MatchOptions, WorklistDir};
use std::sync::Arc;

pub struct BridgeHandler {
    pub cfg: Arc<Config>,
    pub worklist: Arc<WorklistDir>,
    pub archive: Arc<Archive>,
    pub pipeline: Arc<Pipeline>,
}

impl Handler for BridgeHandler {
    fn supports_find(&self) -> bool {
        true
    }

    fn supports_store(&self) -> bool {
        true
    }

    fn on_find(&self, peer: &PeerInfo, identifier: &Dataset) -> Result<Vec<Dataset>, u16> {
        let mut query = identifier.clone();
        // Ohne konfigurierte Station gilt der Eintrag für jedes Gerät.
        if self.cfg.station_ae.is_empty() {
            if let Some(mut sps) = query.get_seq(t::SPS_SEQUENCE).map(|s| s.to_vec()) {
                for item in &mut sps {
                    item.remove(t::SPS_STATION_AE);
                    item.remove(t::SPS_STATION_NAME);
                }
                query.set_seq(t::SPS_SEQUENCE, sps);
            }
        }
        match self.worklist.query(
            &query,
            MatchOptions {
                match_dates: self.cfg.match_dates,
            },
        ) {
            Ok(r) => {
                crate::info!(
                    "Worklist-Abfrage von {} ({}): {} Treffer",
                    peer.calling_ae,
                    peer.addr,
                    r.len()
                );
                Ok(r)
            }
            Err(e) => {
                crate::error!("Worklist nicht lesbar: {e}");
                Err(STATUS_PROCESSING_FAILURE)
            }
        }
    }

    fn on_store(&self, peer: &PeerInfo, obj: StoredObject) -> u16 {
        match self.archive.save(&obj) {
            Ok(saved) => {
                crate::info!(
                    "Instanz {} von {} archiviert (Patient {}, Modality {})",
                    obj.sop_instance,
                    peer.calling_ae,
                    saved.meta.patient_id,
                    saved.meta.modality
                );
                self.pipeline.add(&saved);
                STATUS_SUCCESS
            }
            Err(opengdt_store::Error::Io(e)) => {
                crate::error!("Archivierung fehlgeschlagen: {e}");
                STATUS_REFUSED_OUT_OF_RESOURCES
            }
            Err(e) => {
                crate::error!("Instanz {} nicht verarbeitbar: {e}", obj.sop_instance);
                0xC000 // Cannot understand
            }
        }
    }

    fn log(&self, msg: &str) {
        crate::warn!("{msg}");
    }
}
