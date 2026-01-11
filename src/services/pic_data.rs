use crate::db::Database;
use crate::proto::timecard::{
    pic_data_service_server::PicDataService, PaginationRequest, PicData, PicDataList, PicIcData,
    PicIcList, PicTmpData, PicTmpList,
};
use base64::Engine;
use chrono::{Duration, Local};
use sqlx::Row;
use tonic::{Request, Response, Status};

pub struct PicDataServiceImpl {
    db: Database,
}

impl PicDataServiceImpl {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    fn get_default_start_date() -> String {
        let two_days_ago = Local::now() - Duration::days(2);
        two_days_ago.format("%Y-%m-%d %H:%M:%S").to_string()
    }
}

#[tonic::async_trait]
impl PicDataService for PicDataServiceImpl {
    async fn get_all(
        &self,
        _request: Request<()>,
    ) -> Result<Response<PicDataList>, Status> {
        let rows = sqlx::query(
            "SELECT date, cam, pic, detail, machine_ip
             FROM pic_data
             ORDER BY date DESC",
        )
        .fetch_all(self.db.pool())
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let pics: Vec<PicData> = rows
            .iter()
            .map(|row| {
                let date: chrono::NaiveDateTime = row.get("date");
                let pic: Vec<u8> = row.get("pic");
                PicData {
                    date: date.format("%Y-%m-%d %H:%M:%S").to_string(),
                    cam: row.get("cam"),
                    pic_base64: base64::engine::general_purpose::STANDARD.encode(&pic),
                    detail: row.get("detail"),
                    machine_ip: row.get("machine_ip"),
                }
            })
            .collect();

        Ok(Response::new(PicDataList { pics }))
    }

    async fn get_tmp(
        &self,
        request: Request<PaginationRequest>,
    ) -> Result<Response<PicTmpList>, Status> {
        let req = request.into_inner();
        let limit = req.limit.unwrap_or(500);
        let start_date = req
            .start_date
            .unwrap_or_else(Self::get_default_start_date);

        // 複雑なJOINクエリ: tmp_data + pic_data + drivers
        let query = r#"
            SELECT
                s9.*,
                s8.name
            FROM (
                SELECT
                    s7.*,
                    s6.pic as pic_2,
                    s6.detail as detail_2
                FROM (
                    SELECT
                        s5.*,
                        s4.pic as pic_1
                    FROM (
                        SELECT
                            s3.*,
                            s2.id as driver_id
                        FROM (
                            SELECT tmp, amb, dist, date, machine_ip
                            FROM tmp_data
                            WHERE id = 0
                        ) s3
                        LEFT JOIN (SELECT * FROM tmp_data WHERE id > 0) s2
                            ON s3.machine_ip = s2.machine_ip AND s3.date = s2.date
                    ) s5
                    LEFT JOIN (SELECT * FROM pic_data WHERE detail = 'tmp inserted') s4
                        ON s5.machine_ip = s4.machine_ip AND s5.date = s4.date
                ) s7
                LEFT JOIN (
                    SELECT * FROM pic_data
                    WHERE detail = 'tmp inserted by ic' OR detail = 'tmp inserted by fing'
                ) s6
                    ON s7.machine_ip = s6.machine_ip AND s7.date = s6.date
            ) s9
            LEFT JOIN drivers s8 ON s9.driver_id = s8.id
            WHERE s9.date >= ?
            ORDER BY s9.date DESC
            LIMIT ?
        "#;

        let rows = sqlx::query(query)
            .bind(&start_date)
            .bind(limit)
            .fetch_all(self.db.pool())
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let data: Vec<PicTmpData> = rows
            .iter()
            .map(|row| {
                let date: chrono::NaiveDateTime = row.get("date");
                let pic_1: Option<Vec<u8>> = row.try_get("pic_1").ok();
                let pic_2: Option<Vec<u8>> = row.try_get("pic_2").ok();

                PicTmpData {
                    machine_ip: row.get("machine_ip"),
                    tmp: row.get("tmp"),
                    amb: row.get("amb"),
                    dist: row.get("dist"),
                    date: date.format("%Y-%m-%d %H:%M:%S").to_string(),
                    driver_id: row.try_get("driver_id").ok(),
                    driver_name: row.try_get("name").ok(),
                    pic_data_1: pic_1
                        .map(|p| base64::engine::general_purpose::STANDARD.encode(&p)),
                    pic_data_2: pic_2
                        .map(|p| base64::engine::general_purpose::STANDARD.encode(&p)),
                }
            })
            .collect();

        Ok(Response::new(PicTmpList { data }))
    }

    async fn get_ic(
        &self,
        request: Request<PaginationRequest>,
    ) -> Result<Response<PicIcList>, Status> {
        let req = request.into_inner();
        let limit = req.limit.unwrap_or(500);
        let start_date = req
            .start_date
            .unwrap_or_else(Self::get_default_start_date);

        let query = r#"
            SELECT
                ic.id, ic.type, ic.detail, ic.date, ic.iid, ic.machine_ip,
                p.pic
            FROM ic_log ic
            LEFT JOIN pic_data p ON ic.machine_ip = p.machine_ip AND ic.date = p.date
            WHERE ic.date >= ?
            ORDER BY ic.date DESC
            LIMIT ?
        "#;

        let rows = sqlx::query(query)
            .bind(&start_date)
            .bind(limit)
            .fetch_all(self.db.pool())
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let data: Vec<PicIcData> = rows
            .iter()
            .map(|row| {
                let date: chrono::NaiveDateTime = row.get("date");
                let pic: Option<Vec<u8>> = row.try_get("pic").ok();

                PicIcData {
                    id: row.get("id"),
                    r#type: row.get("type"),
                    detail: row.try_get("detail").ok(),
                    date: date.format("%Y-%m-%d %H:%M:%S").to_string(),
                    iid: row.try_get("iid").ok(),
                    machine_ip: row.get("machine_ip"),
                    pic_base64: pic
                        .map(|p| base64::engine::general_purpose::STANDARD.encode(&p)),
                }
            })
            .collect();

        Ok(Response::new(PicIcList { data }))
    }
}
