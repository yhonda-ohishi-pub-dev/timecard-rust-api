// Socket.IO Server implementation
// Replaces Node.js Socket.IO server on port 3050

use crate::client_state::ClientState;
use crate::db::Database;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use socketioxide::{
    extract::{Data, SocketRef, State},
    SocketIo,
};
use sqlx::Row;
use std::sync::Arc;
use tracing::{error, info, warn};

/// Shared state for Socket.IO handlers
#[derive(Clone)]
pub struct SocketState {
    pub db: Database,
    pub clients: ClientState,
    pub cf_broadcast_url: Option<Arc<String>>,
    pub http_client: reqwest::Client,
}

/// Message data structure from Python client
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageData {
    pub ip: Option<String>,
    pub status: Option<String>,
    pub message: Option<String>,
    pub data: Option<MessagePayload>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagePayload {
    pub time: Option<String>,
    pub id: Option<i32>,
    pub name: Option<String>,
    pub tmp: Option<String>,
    pub pic_data: Option<String>,
    pub pic_data_1: Option<String>,
    pub pic_data_2: Option<String>,
}

/// Setup Socket.IO server with message handling
pub fn setup_socketio(
    db: Database,
    clients: ClientState,
    cf_broadcast_url: Option<String>,
) -> (socketioxide::layer::SocketIoLayer, SocketIo) {
    let http_client = reqwest::Client::new();
    let state = SocketState {
        db,
        clients,
        cf_broadcast_url: cf_broadcast_url.map(|url| Arc::new(url)),
        http_client,
    };
    let (layer, io) = SocketIo::builder().with_state(state).build_layer();

    io.ns("/", on_connect);

    (layer, io)
}

/// Handle new socket connection
async fn on_connect(socket: SocketRef, state: State<SocketState>) {
    let socket_id = socket.id.to_string();
    info!("Client connected: {}", socket_id);

    // Register client immediately on connect (IP will be updated on start_connect)
    state.clients.add_client(socket_id.clone(), "unknown".to_string());
    info!("Client registered on connect: {}", socket_id);

    // Send initial hello message on connect
    if let Err(e) = socket.emit("hello", "from server") {
        warn!("Failed to send initial hello: {}", e);
    }

    // Handle message event from Python client
    socket.on(
        "message",
        |socket: SocketRef, Data::<Value>(data), state: State<SocketState>| async move {
            let socket_id = socket.id.to_string();

            // Update last activity for this client
            state.clients.update_activity(&socket_id);

            // Update client IP from any message that contains ip field
            if let Some(ip) = data.get("ip").and_then(|v| v.as_str()) {
                if ip != "unknown" && !ip.is_empty() {
                    state.clients.update_ip(&socket_id, ip.to_string());
                    info!("Client IP updated: {} -> {}", socket_id, ip);
                }
            }

            info!("Received message: {:?}", data);
            handle_message(
                socket,
                data,
                state.db.clone(),
                state.cf_broadcast_url.clone(),
                state.http_client.clone(),
            )
            .await;
        },
    );

    // Handle disconnect
    socket.on_disconnect(|socket: SocketRef, state: State<SocketState>| async move {
        let socket_id = socket.id.to_string();
        if let Some(client) = state.clients.remove_client(&socket_id) {
            info!(
                "Python client disconnected: {} ({})",
                socket_id, client.ip_address
            );
        } else {
            info!("Client disconnected: {}", socket_id);
        }
    });
}

/// Process message and broadcast hello event
async fn handle_message(
    socket: SocketRef,
    mut data: Value,
    db: Database,
    cf_broadcast_url: Option<Arc<String>>,
    http_client: reqwest::Client,
) {
    let status = data
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    match status.as_str() {
        "tmp inserted wo pic" => {
            // Get driver name from database if id is provided but name is missing
            if let Some(inner_data) = data.get_mut("data") {
                let has_id = inner_data.get("id").is_some();
                let has_name = inner_data.get("name").and_then(|v| v.as_str()).is_some();

                if has_id && !has_name {
                    if let Some(id) = inner_data.get("id").and_then(|v| v.as_i64()) {
                        match get_driver_name(&db, id as i32).await {
                            Ok(Some(name)) => {
                                inner_data["name"] = json!(name);
                                info!("Added driver name {} for id {}", name, id);
                            }
                            Ok(None) => {
                                warn!("Driver not found for id {}", id);
                            }
                            Err(e) => {
                                error!("Failed to fetch driver name: {}", e);
                            }
                        }
                    }
                }
            }
        }
        "tmp inserted" | "tmp inserted by ic" | "tmp inserted by fing" => {
            // These messages may contain pic_data - pass through as is
            // Base64 encoding is already done by Python client
            info!("Processing {} event", status);
        }
        "insert ic_log" => {
            info!("IC log event received");
        }
        "delete_ic" => {
            info!("Delete IC event received");
        }
        _ => {
            info!("Unknown status: {}, broadcasting as-is", status);
        }
    }

    // Broadcast hello event to all clients (including sender)
    let json_str = serde_json::to_string(&data).unwrap_or_else(|_| "{}".to_string());
    broadcast_hello(&socket, &json_str).await;

    // Notify Cloudflare Worker asynchronously (fire-and-forget)
    if let Some(url) = cf_broadcast_url {
        let json_str_clone = json_str.clone();
        tokio::spawn(async move {
            notify_cf_worker(&http_client, &url, &json_str_clone).await;
        });
    }
}

/// Get driver name from database
async fn get_driver_name(
    db: &Database,
    driver_id: i32,
) -> Result<Option<String>, sqlx::Error> {
    let row = sqlx::query("SELECT name FROM drivers WHERE id = ?")
        .bind(driver_id)
        .fetch_optional(db.pool())
        .await?;

    Ok(row.map(|r| r.get("name")))
}

/// Broadcast hello event to all connected clients
async fn broadcast_hello(socket: &SocketRef, data: &str) {
    // Broadcast to all other clients
    if let Err(e) = socket.broadcast().emit("hello", data) {
        error!("Failed to broadcast hello: {}", e);
    }

    // Also send to the sender
    if let Err(e) = socket.emit("hello", data) {
        error!("Failed to emit hello to sender: {}", e);
    }

    info!("Broadcasted hello event");
}

/// Notify Cloudflare Worker to broadcast message to WebSocket clients
async fn notify_cf_worker(client: &reqwest::Client, url: &str, data: &str) {
    // Wrap data in hello event format for frontend
    let payload = json!({
        "type": "hello",
        "data": serde_json::from_str::<Value>(data).unwrap_or(Value::Null),
        "timestamp": chrono::Utc::now().to_rfc3339()
    });

    match client
        .post(url)
        .header("Content-Type", "application/json")
        .body(payload.to_string())
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
    {
        Ok(resp) => {
            if resp.status().is_success() {
                info!("CF Worker notified successfully");
            } else {
                warn!("CF Worker returned status: {}", resp.status());
            }
        }
        Err(e) => {
            warn!("Failed to notify CF Worker: {}", e);
        }
    }
}

/// Get SocketIo instance for external use (e.g., emit from HTTP handlers)
#[allow(dead_code)]
pub struct SocketIoHandle {
    io: SocketIo,
}

#[allow(dead_code)]
impl SocketIoHandle {
    pub fn new(io: SocketIo) -> Self {
        Self { io }
    }

    /// Emit hello event to all connected clients
    pub async fn emit_hello(&self, data: &str) -> Result<(), String> {
        self.io
            .of("/")
            .ok_or_else(|| "Namespace not found".to_string())?
            .emit("hello", data)
            .map_err(|e| e.to_string())
    }

    /// Emit delete_ic event
    pub async fn emit_delete_ic(&self, ic_id: &str) -> Result<(), String> {
        let data = json!({
            "status": "delete_ic",
            "ic": ic_id
        });
        let json_str = serde_json::to_string(&data).map_err(|e| e.to_string())?;
        self.emit_hello(&json_str).await
    }
}
