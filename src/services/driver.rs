use crate::db::Database;
use crate::proto::timecard::{
    driver_service_server::DriverService, Driver, DriverIdRequest, DriverList,
};
use sqlx::Row;
use tonic::{Request, Response, Status};

pub struct DriverServiceImpl {
    db: Database,
}

impl DriverServiceImpl {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

#[tonic::async_trait]
impl DriverService for DriverServiceImpl {
    async fn get_all(
        &self,
        _request: Request<()>,
    ) -> Result<Response<DriverList>, Status> {
        let rows = sqlx::query("SELECT id, name FROM drivers")
            .fetch_all(self.db.pool())
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let drivers: Vec<Driver> = rows
            .iter()
            .map(|row| Driver {
                id: row.get("id"),
                name: row.get("name"),
            })
            .collect();

        Ok(Response::new(DriverList { drivers }))
    }

    async fn get_by_id(
        &self,
        request: Request<DriverIdRequest>,
    ) -> Result<Response<Driver>, Status> {
        let driver_id = request.into_inner().driver_id;

        let row = sqlx::query("SELECT id, name FROM drivers WHERE id = ? LIMIT 1")
            .bind(driver_id)
            .fetch_optional(self.db.pool())
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        match row {
            Some(row) => Ok(Response::new(Driver {
                id: row.get("id"),
                name: row.get("name"),
            })),
            None => Err(Status::not_found(format!(
                "Driver with id {} not found",
                driver_id
            ))),
        }
    }

    async fn reload(
        &self,
        _request: Request<()>,
    ) -> Result<Response<DriverList>, Status> {
        // 外部APIからドライバーデータを取得
        // 注: 本番環境では実際のAPIエンドポイントを使用
        let external_api_url = "http://172.18.21.35:85/drivers/names";

        let client = reqwest::Client::new();
        let response = client
            .get(external_api_url)
            .send()
            .await
            .map_err(|e| Status::internal(format!("Failed to fetch from external API: {}", e)))?;

        if !response.status().is_success() {
            return Err(Status::internal("External API returned error"));
        }

        #[derive(serde::Deserialize)]
        struct ExternalDriver {
            id: i32,
            name: String,
        }

        let external_drivers: Vec<ExternalDriver> = response
            .json()
            .await
            .map_err(|e| Status::internal(format!("Failed to parse external API response: {}", e)))?;

        // トランザクションで既存データを削除して新しいデータを挿入
        let mut tx = self
            .db
            .pool()
            .begin()
            .await
            .map_err(|e| Status::internal(format!("Transaction error: {}", e)))?;

        sqlx::query("DELETE FROM drivers")
            .execute(&mut *tx)
            .await
            .map_err(|e| Status::internal(format!("Delete error: {}", e)))?;

        for driver in &external_drivers {
            sqlx::query("INSERT INTO drivers (id, name) VALUES (?, ?)")
                .bind(driver.id)
                .bind(&driver.name)
                .execute(&mut *tx)
                .await
                .map_err(|e| Status::internal(format!("Insert error: {}", e)))?;
        }

        tx.commit()
            .await
            .map_err(|e| Status::internal(format!("Commit error: {}", e)))?;

        let drivers: Vec<Driver> = external_drivers
            .into_iter()
            .map(|d| Driver {
                id: d.id,
                name: d.name,
            })
            .collect();

        Ok(Response::new(DriverList { drivers }))
    }
}
