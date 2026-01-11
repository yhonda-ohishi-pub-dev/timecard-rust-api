use crate::db::Database;
use crate::proto::timecard::{
    tmp_data_service_server::TmpDataService, PaginationRequest, TmpData, TmpDataList,
};
use chrono::{Duration, Local};
use sqlx::Row;
use tonic::{Request, Response, Status};

pub struct TmpDataServiceImpl {
    db: Database,
}

impl TmpDataServiceImpl {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    fn get_default_start_date() -> String {
        let two_days_ago = Local::now() - Duration::days(2);
        two_days_ago.format("%Y-%m-%d %H:%M:%S").to_string()
    }
}

#[tonic::async_trait]
impl TmpDataService for TmpDataServiceImpl {
    async fn get_all(
        &self,
        request: Request<PaginationRequest>,
    ) -> Result<Response<TmpDataList>, Status> {
        let req = request.into_inner();
        let limit = req.limit.unwrap_or(500);

        let rows = sqlx::query(
            "SELECT machine_ip, tmp, amb, dist, date, id
             FROM tmp_data
             WHERE id = 0
             ORDER BY date DESC
             LIMIT ?",
        )
        .bind(limit)
        .fetch_all(self.db.pool())
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let data: Vec<TmpData> = rows
            .iter()
            .map(|row| {
                let date: chrono::NaiveDateTime = row.get("date");
                TmpData {
                    machine_ip: row.get("machine_ip"),
                    tmp: row.get("tmp"),
                    amb: row.get("amb"),
                    dist: row.get("dist"),
                    date: date.format("%Y-%m-%d %H:%M:%S").to_string(),
                    id: row.get("id"),
                }
            })
            .collect();

        Ok(Response::new(TmpDataList { data }))
    }

    async fn get_without_pic(
        &self,
        request: Request<PaginationRequest>,
    ) -> Result<Response<TmpDataList>, Status> {
        let req = request.into_inner();
        let limit = req.limit.unwrap_or(500);
        let start_date = req
            .start_date
            .unwrap_or_else(Self::get_default_start_date);

        let rows = sqlx::query(
            "SELECT t.machine_ip, t.tmp, t.amb, t.dist, t.date, t.id
             FROM tmp_data t
             LEFT JOIN pic_data p ON t.machine_ip = p.machine_ip AND t.date = p.date
             WHERE p.machine_ip IS NULL AND t.date >= ?
             ORDER BY t.date DESC
             LIMIT ?",
        )
        .bind(&start_date)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let data: Vec<TmpData> = rows
            .iter()
            .map(|row| {
                let date: chrono::NaiveDateTime = row.get("date");
                TmpData {
                    machine_ip: row.get("machine_ip"),
                    tmp: row.get("tmp"),
                    amb: row.get("amb"),
                    dist: row.get("dist"),
                    date: date.format("%Y-%m-%d %H:%M:%S").to_string(),
                    id: row.get("id"),
                }
            })
            .collect();

        Ok(Response::new(TmpDataList { data }))
    }
}
