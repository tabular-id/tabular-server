use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::sync::broadcast;
use yrs::{Doc, ReadTxn, Transact};

/// A single collaborative room held in memory.
pub struct CollabRoom {
    pub room_id: String,
    /// Yjs document — source of truth for the text
    pub doc: Arc<Mutex<Doc>>,
    /// Broadcast channel: any WS update is relayed to all connected clients
    pub tx: broadcast::Sender<Vec<u8>>,
    /// Number of currently connected clients
    pub client_count: Arc<Mutex<usize>>,
}

impl CollabRoom {
    pub fn new(room_id: String) -> Self {
        let (tx, _) = broadcast::channel(512);
        CollabRoom {
            room_id,
            doc: Arc::new(Mutex::new(Doc::new())),
            tx,
            client_count: Arc::new(Mutex::new(0)),
        }
    }

    /// Return a Yjs state vector update (full state) for the "sync step 2" handshake.
    pub fn encode_state(&self) -> Vec<u8> {
        let doc = self.doc.lock().unwrap();
        let txn = doc.transact();
        txn.encode_state_as_update_v1(&Default::default())
    }

    /// Apply a received binary update to the document.
    pub fn apply_update(&self, update: &[u8]) -> Result<(), String> {
        let doc = self.doc.lock().unwrap();
        let mut txn = doc.transact_mut();
        yrs::updates::decoder::Decode::decode_v1(update)
            .map_err(|e| e.to_string())
            .and_then(|u| txn.apply_update(u).map_err(|e| e.to_string()))
    }
}

/// In-memory room registry shared across all WebSocket connections.
#[derive(Clone, Default)]
pub struct RoomRegistry {
    pub rooms: Arc<Mutex<HashMap<String, Arc<CollabRoom>>>>,
}

impl RoomRegistry {
    pub fn new() -> Self {
        RoomRegistry {
            rooms: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn get_or_create(&self, room_id: &str) -> Arc<CollabRoom> {
        let mut rooms = self.rooms.lock().unwrap();
        rooms
            .entry(room_id.to_string())
            .or_insert_with(|| Arc::new(CollabRoom::new(room_id.to_string())))
            .clone()
    }

    pub fn get(&self, room_id: &str) -> Option<Arc<CollabRoom>> {
        self.rooms.lock().unwrap().get(room_id).cloned()
    }

    pub fn remove_if_empty(&self, room_id: &str) {
        let mut rooms = self.rooms.lock().unwrap();
        if let Some(room) = rooms.get(room_id) {
            let count = *room.client_count.lock().unwrap();
            if count == 0 {
                rooms.remove(room_id);
            }
        }
    }
}
