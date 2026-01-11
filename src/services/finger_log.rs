use crate::db::Database;
use crate::proto::timecard::{
    finger_log_service_server::FingerLogService, FingerLog, FingerLogList, TimeRangeRequest,
};
use chrono::{Duration, Local};
use sqlx::Row;
use tonic::{Request, Response, Status};

pub struct FingerLogServiceImpl {
    db: Database,
}

impl FingerLogServiceImpl {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    fn get_default_start_date() -> String {
        let two_days_ago = Local::now() - Duration::days(2);
        two_days_ago.format("%Y-%m-%d %H:%M:%S").to_string()
    }
}

#[tonic::async_trait]
impl FingerLogService for FingerLogServiceImpl {
    async fn get_recent(
        &self,
        request: Request<TimeRangeRequest>,
    ) -> Result<Response<FingerLogList>, Status> {
        let req = request.into_inner();
        let start_date = req
            .start_date
            .unwrap_or_else(Self::get_default_start_date);

        let rows = sqlx::query(
            "SELECT date, machine_ip, id, message
             FROM finger_log
             WHERE date >= ?
             ORDER BY date DESC",
        )
        .bind(&start_date)
        .fetch_all(self.db.pool())
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let logs: Vec<FingerLog> = rows
            .iter()
            .map(|row| {
                let date: chrono::NaiveDateTime = row.get("date");
                FingerLog {
                    date: date.format("%Y-%m-%d %H:%M:%S").to_string(),
                    machine_ip: row.get("machine_ip"),
                    id: row.get("id"),
                    message: row.get("message"),
                }
            })
            .collect();

        Ok(Response::new(FingerLogList { logs }))
    }
}
