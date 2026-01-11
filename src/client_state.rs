// Client state management for tracking connected Socket.IO clients
// Uses DashMap for thread-safe concurrent access

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Information about a connected client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub socket_id: String,
    pub ip_address: String,
    pub connected_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
}

/// Thread-safe state for tracking connected clients
#[derive(Clone)]
pub struct ClientState {
    clients: Arc<DashMap<String, ClientInfo>>,
}

impl ClientState {
    /// Create a new ClientState
    pub fn new() -> Self {
        Self {
            clients: Arc::new(DashMap::new()),
        }
    }

    /// Add a new client to the state
    pub fn add_client(&self, socket_id: String, ip_address: String) {
        let now = Utc::now();
        self.clients.insert(
            socket_id.clone(),
            ClientInfo {
                socket_id,
                ip_address,
                connected_at: now,
                last_activity: now,
            },
        );
    }

    /// Remove a client from the state
    pub fn remove_client(&self, socket_id: &str) -> Option<ClientInfo> {
        self.clients.remove(socket_id).map(|(_, v)| v)
    }

    /// Update the last activity time for a client
    pub fn update_activity(&self, socket_id: &str) {
        if let Some(mut client) = self.clients.get_mut(socket_id) {
            client.last_activity = Utc::now();
        }
    }

    /// Update the IP address for a client
    pub fn update_ip(&self, socket_id: &str, ip_address: String) {
        if let Some(mut client) = self.clients.get_mut(socket_id) {
            client.ip_address = ip_address;
            client.last_activity = Utc::now();
        }
    }

    /// Get all connected clients
    pub fn get_all_clients(&self) -> Vec<ClientInfo> {
        self.clients
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Get the number of connected clients
    pub fn get_client_count(&self) -> usize {
        self.clients.len()
    }
}

impl Default for ClientState {
    fn default() -> Self {
        Self::new()
    }
}
