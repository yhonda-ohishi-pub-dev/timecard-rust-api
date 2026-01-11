use rust_socketio::{
    asynchronous::{Client, ClientBuilder},
    Payload,
};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

pub struct SocketIoClient {
    client: Arc<RwLock<Option<Client>>>,
    url: String,
}

impl SocketIoClient {
    pub async fn new(url: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let socket_client = Self {
            client: Arc::new(RwLock::new(None)),
            url: url.to_string(),
        };
        socket_client.connect().await?;
        Ok(socket_client)
    }

    async fn connect(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Connecting to Socket.IO server: {}", self.url);

        let client = ClientBuilder::new(&self.url)
            .namespace("/")
            .on("connect", |_, _| {
                async {
                    info!("Connected to Socket.IO server");
                }
                .boxed()
            })
            .on("hello", |payload, _| {
                async move {
                    info!("Received hello event: {:?}", payload);
                }
                .boxed()
            })
            .on("error", |err, _| {
                async move {
                    error!("Socket.IO error: {:?}", err);
                }
                .boxed()
            })
            .connect()
            .await?;

        let mut guard = self.client.write().await;
        *guard = Some(client);

        Ok(())
    }

    pub async fn emit_message(
        &self,
        data: serde_json::Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let guard = self.client.read().await;
        if let Some(client) = guard.as_ref() {
            client
                .emit("message", Payload::Text(vec![data]))
                .await?;
            info!("Emitted message event");
            Ok(())
        } else {
            warn!("Socket.IO client not connected");
            Err("Socket.IO client not connected".into())
        }
    }

    pub async fn emit_delete_ic(
        &self,
        ic_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let data = json!({
            "status": "delete_ic",
            "ic": ic_id
        });
        self.emit_message(data).await
    }
}

use futures_util::FutureExt;
