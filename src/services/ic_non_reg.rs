use crate::db::Database;
use crate::proto::timecard::{
    ic_non_reg_service_server::IcNonRegService, CancelIcNonRegRequest, IcNonReg, IcNonRegList,
    RegisterDirectRequest, RegisterDirectResponse, TimeRangeRequest, UpdateIcNonRegRequest,
};
use chrono::{Duration, Local};
use sqlx::Row;
use tonic::{Request, Response, Status};

pub struct ICNonRegServiceImpl {
    db: Database,
}

impl ICNonRegServiceImpl {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    fn get_default_start_date() -> String {
        let two_days_ago = Local::now() - Duration::days(2);
        two_days_ago.format("%Y-%m-%d %H:%M:%S").to_string()
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
            "SELECT id, datetime, deleted, registered_id
             FROM ic_non_reged
             WHERE datetime >= ?
             ORDER BY datetime DESC",
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

        // ic_non_regedテーブルを更新
        sqlx::query(
            "UPDATE ic_non_reged
             SET deleted = 1, registered_id = ?
             WHERE id = ?",
        )
        .bind(req.driver_id)
        .bind(&req.ic_id)
        .execute(self.db.pool())
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        // ic_idテーブルに新しいエントリを追加
        sqlx::query(
            "INSERT INTO ic_id (ic_id, emp_id, deleted, date)
             VALUES (?, ?, 0, NOW())",
        )
        .bind(&req.ic_id)
        .bind(req.driver_id)
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

        // registered_idをNULLに戻す
        sqlx::query(
            "UPDATE ic_non_reged
             SET registered_id = NULL
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

        // 2. ICカードが既にic_idテーブルに登録されているか確認
        let existing = sqlx::query("SELECT ic_id FROM ic_id WHERE ic_id = ? AND deleted = 0")
            .bind(&req.ic_id)
            .fetch_optional(self.db.pool())
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        if existing.is_some() {
            return Ok(Response::new(RegisterDirectResponse {
                success: false,
                message: "このICカードは既に登録されています".to_string(),
                ic_id: Some(req.ic_id),
                driver_id: None,
                driver_name: None,
            }));
        }

        // 3. ic_non_regedにregistered_idを設定
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
}
