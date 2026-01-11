use crate::db::Database;
use crate::proto::timecard::{
    ic_log_service_server::IcLogService, IcLog, IcLogList, IcLogWithDriver, IcLogWithDriverList,
    PaginationRequest, TimeRangeRequest,
};
use chrono::{Duration, Local};
use sqlx::Row;
use tonic::{Request, Response, Status};

pub struct ICLogServiceImpl {
    db: Database,
}

impl ICLogServiceImpl {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    fn get_default_start_date() -> String {
        let two_days_ago = Local::now() - Duration::days(2);
        two_days_ago.format("%Y-%m-%d %H:%M:%S").to_string()
    }
}

#[tonic::async_trait]
impl IcLogService for ICLogServiceImpl {
    async fn get_recent(
        &self,
        request: Request<TimeRangeRequest>,
    ) -> Result<Response<IcLogList>, Status> {
        let req = request.into_inner();
        let start_date = req
            .start_date
            .unwrap_or_else(Self::get_default_start_date);

        let rows = sqlx::query(
            "SELECT id, type, detail, date, iid, machine_ip
             FROM ic_log
             WHERE date >= ?
             ORDER BY date ASC",
        )
        .bind(&start_date)
        .fetch_all(self.db.pool())
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let logs: Vec<IcLog> = rows
            .iter()
            .map(|row| {
                let date: chrono::NaiveDateTime = row.get("date");
                IcLog {
                    id: row.get("id"),
                    r#type: row.get("type"),
                    detail: row.get("detail"),
                    date: date.format("%Y-%m-%d %H:%M:%S").to_string(),
                    iid: row.get("iid"),
                    machine_ip: row.get("machine_ip"),
                }
            })
            .collect();

        Ok(Response::new(IcLogList { logs }))
    }

    async fn get_recent_desc(
        &self,
        request: Request<TimeRangeRequest>,
    ) -> Result<Response<IcLogList>, Status> {
        let req = request.into_inner();
        let start_date = req
            .start_date
            .unwrap_or_else(Self::get_default_start_date);

        let rows = sqlx::query(
            "SELECT id, type, detail, date, iid, machine_ip
             FROM ic_log
             WHERE date >= ?
             ORDER BY date DESC",
        )
        .bind(&start_date)
        .fetch_all(self.db.pool())
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let logs: Vec<IcLog> = rows
            .iter()
            .map(|row| {
                let date: chrono::NaiveDateTime = row.get("date");
                IcLog {
                    id: row.get("id"),
                    r#type: row.get("type"),
                    detail: row.get("detail"),
                    date: date.format("%Y-%m-%d %H:%M:%S").to_string(),
                    iid: row.get("iid"),
                    machine_ip: row.get("machine_ip"),
                }
            })
            .collect();

        Ok(Response::new(IcLogList { logs }))
    }

    async fn get_with_driver(
        &self,
        request: Request<TimeRangeRequest>,
    ) -> Result<Response<IcLogWithDriverList>, Status> {
        let req = request.into_inner();
        let start_date = req
            .start_date
            .unwrap_or_else(Self::get_default_start_date);

        // ドライバー名取得: ic_id経由またはic_log.iid直接参照（免許証の場合）
        // 同一ICカードに複数レコードがある場合は最新のみを使用
        let rows = sqlx::query(
            "SELECT ic.id, ic.type, ic.detail, ic.date, ic.iid, ic.machine_ip,
                    COALESCE(d1.name, d2.name) as name
             FROM ic_log ic
             LEFT JOIN (
                 SELECT i1.ic_id, i1.emp_id
                 FROM ic_id i1
                 INNER JOIN (
                     SELECT ic_id, MAX(date) as max_date
                     FROM ic_id
                     WHERE deleted = 0 AND ic_id != ''
                     GROUP BY ic_id
                 ) i2 ON i1.ic_id = i2.ic_id AND i1.date = i2.max_date
                 WHERE i1.deleted = 0
             ) i ON ic.id = i.ic_id
             LEFT JOIN drivers d1 ON i.emp_id = d1.id
             LEFT JOIN drivers d2 ON ic.iid = d2.id
             WHERE ic.date >= ?
             ORDER BY ic.date DESC",
        )
        .bind(&start_date)
        .fetch_all(self.db.pool())
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let logs: Vec<IcLogWithDriver> = rows
            .iter()
            .map(|row| {
                let date: chrono::NaiveDateTime = row.get("date");
                IcLogWithDriver {
                    id: row.get("id"),
                    r#type: row.get("type"),
                    detail: row.get("detail"),
                    date: date.format("%Y-%m-%d %H:%M:%S").to_string(),
                    iid: row.get("iid"),
                    machine_ip: row.get("machine_ip"),
                    driver_name: row.get("name"),
                }
            })
            .collect();

        Ok(Response::new(IcLogWithDriverList { logs }))
    }

    async fn get_latest_with_driver(
        &self,
        request: Request<PaginationRequest>,
    ) -> Result<Response<IcLogWithDriverList>, Status> {
        let req = request.into_inner();
        let limit = req.limit.unwrap_or(100);

        // 最新N件をドライバー名付きで取得
        // ドライバー名取得: ic_id経由またはic_log.iid直接参照（免許証の場合）
        // 同一ICカードに複数レコードがある場合は最新のみを使用
        let rows = sqlx::query(
            "SELECT ic.id, ic.type, ic.detail, ic.date, ic.iid, ic.machine_ip,
                    COALESCE(d1.name, d2.name) as name
             FROM ic_log ic
             LEFT JOIN (
                 SELECT i1.ic_id, i1.emp_id
                 FROM ic_id i1
                 INNER JOIN (
                     SELECT ic_id, MAX(date) as max_date
                     FROM ic_id
                     WHERE deleted = 0 AND ic_id != ''
                     GROUP BY ic_id
                 ) i2 ON i1.ic_id = i2.ic_id AND i1.date = i2.max_date
                 WHERE i1.deleted = 0
             ) i ON ic.id = i.ic_id
             LEFT JOIN drivers d1 ON i.emp_id = d1.id
             LEFT JOIN drivers d2 ON ic.iid = d2.id
             ORDER BY ic.date DESC
             LIMIT ?",
        )
        .bind(limit)
        .fetch_all(self.db.pool())
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let logs: Vec<IcLogWithDriver> = rows
            .iter()
            .map(|row| {
                let date: chrono::NaiveDateTime = row.get("date");
                IcLogWithDriver {
                    id: row.get("id"),
                    r#type: row.get("type"),
                    detail: row.get("detail"),
                    date: date.format("%Y-%m-%d %H:%M:%S").to_string(),
                    iid: row.get("iid"),
                    machine_ip: row.get("machine_ip"),
                    driver_name: row.get("name"),
                }
            })
            .collect();

        Ok(Response::new(IcLogWithDriverList { logs }))
    }

    async fn get_without_tmp(
        &self,
        request: Request<PaginationRequest>,
    ) -> Result<Response<IcLogList>, Status> {
        let req = request.into_inner();
        let limit = req.limit.unwrap_or(500);
        let start_date = req
            .start_date
            .unwrap_or_else(Self::get_default_start_date);

        let rows = sqlx::query(
            "SELECT ic.id, ic.type, ic.detail, ic.date, ic.iid, ic.machine_ip
             FROM ic_log ic
             LEFT JOIN tmp_data t ON ic.machine_ip = t.machine_ip AND ic.date = t.date
             WHERE t.machine_ip IS NULL AND ic.date >= ?
             ORDER BY ic.date DESC
             LIMIT ?",
        )
        .bind(&start_date)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let logs: Vec<IcLog> = rows
            .iter()
            .map(|row| {
                let date: chrono::NaiveDateTime = row.get("date");
                IcLog {
                    id: row.get("id"),
                    r#type: row.get("type"),
                    detail: row.get("detail"),
                    date: date.format("%Y-%m-%d %H:%M:%S").to_string(),
                    iid: row.get("iid"),
                    machine_ip: row.get("machine_ip"),
                }
            })
            .collect();

        Ok(Response::new(IcLogList { logs }))
    }
}
