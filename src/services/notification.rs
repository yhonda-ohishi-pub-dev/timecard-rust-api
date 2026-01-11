use crate::db::Database;
use crate::proto::timecard::{
    notification_service_server::NotificationService, EventData, TimeCardEvent,
};
use base64::Engine;
use sqlx::Row;
use std::sync::Arc;
use tokio::sync::broadcast;
use tonic::{Request, Response, Status};

/// タイムカードイベントをブロードキャストするためのチャンネル
pub type EventBroadcaster = broadcast::Sender<TimeCardEvent>;

pub struct NotificationServiceImpl {
    db: Database,
    broadcaster: Arc<EventBroadcaster>,
}

impl NotificationServiceImpl {
    pub fn new(db: Database, broadcaster: Arc<EventBroadcaster>) -> Self {
        Self { db, broadcaster }
    }
}

#[tonic::async_trait]
impl NotificationService for NotificationServiceImpl {
    /// Cloudflare DOからのイベントをブロードキャスト
    async fn broadcast_event(
        &self,
        request: Request<TimeCardEvent>,
    ) -> Result<Response<()>, Status> {
        let event = request.into_inner();

        // ブロードキャストチャンネルに送信
        let _ = self.broadcaster.send(event);

        Ok(Response::new(()))
    }

    /// ドライバー名を解決してイベントを返す
    /// "tmp inserted wo pic" ステータスの場合、IDからドライバー名を取得
    async fn resolve_and_broadcast(
        &self,
        request: Request<TimeCardEvent>,
    ) -> Result<Response<TimeCardEvent>, Status> {
        let mut event = request.into_inner();

        // ステータスが "tmp inserted wo pic" の場合、ドライバー名を解決
        if event.status == "tmp inserted wo pic" {
            if let Some(ref mut data) = event.data {
                if data.id != 0 && data.name.is_empty() {
                    // データベースからドライバー名を取得
                    let row = sqlx::query("SELECT name FROM drivers WHERE id = ? LIMIT 1")
                        .bind(data.id)
                        .fetch_optional(self.db.pool())
                        .await
                        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

                    if let Some(row) = row {
                        data.name = row.get("name");
                    }
                }

                // pic_dataをbase64に変換
                if let Some(ref pic_data) = data.pic_data {
                    if !pic_data.is_empty() {
                        data.pic_data_base64 = Some(
                            base64::engine::general_purpose::STANDARD.encode(pic_data),
                        );
                    }
                }
            }
        }

        // ブロードキャストチャンネルに送信
        let _ = self.broadcaster.send(event.clone());

        Ok(Response::new(event))
    }
}
