use crate::db::Database;
use crate::proto::timecard::{
    ic_non_reg_service_server::IcNonRegService, CancelIcNonRegRequest, DeleteIcRequest,
    DeleteIcResponse, IcNonReg, IcNonRegList, RegisterDirectRequest, RegisterDirectResponse,
    TimeRangeRequest, UpdateIcNonRegRequest,
};
use chrono::{Duration, Local};
use serde_json::json;
use socketioxide::SocketIo;
use sqlx::Row;
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct ICNonRegServiceImpl {
    db: Database,
    socketio: Option<Arc<SocketIo>>,
}

impl ICNonRegServiceImpl {
    pub fn new(db: Database) -> Self {
        Self { db, socketio: None }
    }

    pub fn with_socketio(db: Database, socketio: Arc<SocketIo>) -> Self {
        Self {
            db,
            socketio: Some(socketio),
        }
    }

    fn get_default_start_date() -> String {
        let one_hour_ago = Local::now() - Duration::hours(1);
        one_hour_ago.format("%Y-%m-%d %H:%M:%S").to_string()
    }
}

#[tonic::async_trait]
impl IcNonRegService for ICNonRegServiceImpl {
    async fn get_all(
        &self,
        request: Request<TimeRangeRequest>,
    ) -> Result<Response<IcNonRegList>, Status> {
        let req = request.into_inner();
        let start_date = req
            .start_date
            .unwrap_or_else(Self::get_default_start_date);

        let rows = sqlx::query(
            "SELECT n.id, n.datetime, n.deleted, n.registered_id
             FROM ic_non_reged n
             LEFT JOIN ic_id i ON n.id = i.ic_id
               AND (i.deleted = 0 OR i.deleted IS NULL)
               AND i.date >= n.datetime
             WHERE n.datetime >= ? AND (n.deleted = 0 OR n.deleted IS NULL)
               AND i.ic_id IS NULL
             ORDER BY n.datetime DESC",
        )
        .bind(&start_date)
        .fetch_all(self.db.pool())
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let items: Vec<IcNonReg> = rows
            .iter()
            .map(|row| {
                let datetime: chrono::NaiveDateTime = row.get("datetime");
                let deleted: Option<i8> = row.try_get("deleted").ok();
                IcNonReg {
                    id: row.get("id"),
                    datetime: datetime.format("%Y-%m-%d %H:%M:%S").to_string(),
                    deleted: deleted.map(|d| d != 0),
                    registered_id: row.try_get("registered_id").ok(),
                }
            })
            .collect();

        Ok(Response::new(IcNonRegList { items }))
    }

    async fn update(
        &self,
        request: Request<UpdateIcNonRegRequest>,
    ) -> Result<Response<()>, Status> {
        let req = request.into_inner();

        // ic_non_regedテーブルを更新（deleted=0のまま、Pythonクライアントが処理後にdeleted=1にする）
        sqlx::query(
            "UPDATE ic_non_reged
             SET registered_id = ?
             WHERE id = ?",
        )
        .bind(req.driver_id)
        .bind(&req.ic_id)
        .execute(self.db.pool())
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(()))
    }

    async fn cancel_reservation(
        &self,
        request: Request<CancelIcNonRegRequest>,
    ) -> Result<Response<()>, Status> {
        let req = request.into_inner();

        // registered_idをNULLに戻し、deletedも0に戻す
        // これにより一覧に再表示される
        sqlx::query(
            "UPDATE ic_non_reged
             SET registered_id = NULL, deleted = 0
             WHERE id = ?",
        )
        .bind(&req.ic_id)
        .execute(self.db.pool())
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(()))
    }

    async fn register_direct(
        &self,
        request: Request<RegisterDirectRequest>,
    ) -> Result<Response<RegisterDirectResponse>, Status> {
        let req = request.into_inner();

        // 1. ドライバー名を取得（存在しない場合は空文字）
        let driver_row = sqlx::query("SELECT id, name FROM drivers WHERE id = ?")
            .bind(req.driver_id)
            .fetch_optional(self.db.pool())
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let driver_name: String = driver_row
            .map(|row| row.get("name"))
            .unwrap_or_else(|| format!("ID:{}", req.driver_id));

        // 2. ic_non_regedにregistered_idを設定
        // Pythonクライアントが次回ICタッチ時に登録を完了する
        sqlx::query(
            r#"INSERT INTO ic_non_reged (id, registered_id, datetime, deleted)
               VALUES (?, ?, NOW() + INTERVAL 9 HOUR, 0)
               ON DUPLICATE KEY UPDATE
               registered_id = VALUES(registered_id),
               datetime = NOW() + INTERVAL 9 HOUR,
               deleted = 0"#,
        )
        .bind(&req.ic_id)
        .bind(req.driver_id)
        .execute(self.db.pool())
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(RegisterDirectResponse {
            success: true,
            message: "ICカード登録予約完了。次回ICタッチ時に登録されます".to_string(),
            ic_id: Some(req.ic_id),
            driver_id: Some(req.driver_id),
            driver_name: Some(driver_name),
        }))
    }

    async fn delete_ic(
        &self,
        request: Request<DeleteIcRequest>,
    ) -> Result<Response<DeleteIcResponse>, Status> {
        let req = request.into_inner();
        let ic_id = req.ic_id.to_uppercase();

        tracing::info!("Delete IC request received for: {}", ic_id);

        // Socket.IO経由でPythonクライアントにブロードキャスト
        if let Some(ref io) = self.socketio {
            let data = json!({
                "status": "delete_ic",
                "ic": ic_id
            });
            let json_str = serde_json::to_string(&data)
                .map_err(|e| Status::internal(format!("JSON serialization error: {}", e)))?;

            // Pythonクライアントはjson.loads後にtype(data) is strでチェックするため
            // 二重にJSONエンコードして文字列として送信する必要がある
            let double_encoded = serde_json::to_string(&json_str)
                .map_err(|e| Status::internal(format!("JSON serialization error: {}", e)))?;

            if let Some(ns) = io.of("/") {
                if let Err(e) = ns.emit("hello", &double_encoded) {
                    tracing::error!("Failed to emit delete_ic event: {}", e);
                    return Ok(Response::new(DeleteIcResponse {
                        success: false,
                        message: format!("Socket.IO emit failed: {}", e),
                    }));
                }
                tracing::info!("Delete IC event broadcasted: {}", ic_id);
            } else {
                return Ok(Response::new(DeleteIcResponse {
                    success: false,
                    message: "Socket.IO namespace not found".to_string(),
                }));
            }
        } else {
            return Ok(Response::new(DeleteIcResponse {
                success: false,
                message: "Socket.IO not configured".to_string(),
            }));
        }

        Ok(Response::new(DeleteIcResponse {
            success: true,
            message: format!("IC削除リクエストを送信しました: {}", ic_id),
        }))
    }
}
