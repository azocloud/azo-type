use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use yrs::sync::{Awareness, AwarenessUpdate};
use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;
use yrs::Doc;

/// Presence state broadcast by each peer via `yrs::sync::Awareness`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerPresence {
    pub client_id: u64,
    pub endpoint_id: String,
    pub name: String,
    pub color_rgb: [u8; 3],
    pub active_file_id: Option<String>,
    pub active_path: Option<String>,
    /// Primary caret position in `egui` `char` (Unicode scalar) index.
    pub cursor_char: Option<usize>,
    /// Selection anchor position in `egui` `char` (Unicode scalar) index.
    pub anchor_char: Option<usize>,
}

/// Manages `yrs::sync::Awareness` state for local and remote peers.
pub struct CollabAwareness {
    inner: Awareness,
}

impl CollabAwareness {
    pub fn new(doc: Doc) -> Self {
        Self {
            inner: Awareness::new(doc),
        }
    }

    pub fn client_id(&self) -> u64 {
        self.inner.client_id().get()
    }

    pub fn set_local_presence(&mut self, presence: &PeerPresence) -> Result<Vec<u8>, String> {
        self.inner
            .set_local_state(presence)
            .map_err(|e| format!("Failed to set awareness state: {}", e))?;
        self.encode_full_update()
    }

    pub fn encode_full_update(&self) -> Result<Vec<u8>, String> {
        let update = self
            .inner
            .update()
            .map_err(|e| format!("Failed to build awareness update: {}", e))?;
        Ok(update.encode_v1())
    }

    pub fn apply_remote_awareness_bytes(&mut self, bytes: &[u8]) -> Result<(), String> {
        let update = AwarenessUpdate::decode_v1(bytes)
            .map_err(|e| format!("Malformed awareness update: {}", e))?;
        self.inner
            .apply_update(update)
            .map_err(|e| format!("Awareness apply error: {}", e))
    }

    pub fn remove_peer_by_endpoint(&mut self, endpoint_id: &str) {
        let mut to_remove = Vec::new();
        for (cid, _) in self.inner.iter() {
            if let Some(p) = self.inner.state::<PeerPresence>(cid) {
                if p.endpoint_id == endpoint_id {
                    to_remove.push(cid);
                }
            }
        }
        for cid in to_remove {
            self.inner.remove_state(cid);
        }
    }

    /// Returns all active remote peer presence states (excluding our own local `client_id`).
    pub fn remote_peers(&self) -> Vec<PeerPresence> {
        let local_cid = self.inner.client_id();
        let mut by_client: BTreeMap<u64, PeerPresence> = BTreeMap::new();
        for (cid, _) in self.inner.iter() {
            if cid == local_cid {
                continue;
            }
            if let Some(presence) = self.inner.state::<PeerPresence>(cid) {
                by_client.insert(cid.get(), presence);
            }
        }
        by_client.into_values().collect()
    }
}

#[cfg(test)]
mod tests;
