use crate::db::Database;
use crate::proto::timecard::{test_service_server::TestService, TestData, TestDataList};
use sqlx::Row;
use tonic::{Request, Response, Status};

pub struct TestServiceImpl {
    db: Database,
}

impl TestServiceImpl {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

#[tonic::async_trait]
impl TestService for TestServiceImpl {
    async fn get_test_data(
        &self,
        _request: Request<()>,
    ) -> Result<Response<TestDataList>, Status> {
        let rows = sqlx::query("SELECT id, datettime FROM test")
            .fetch_all(self.db.pool())
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let data: Vec<TestData> = rows
            .iter()
            .map(|row| TestData {
                id: row.get("id"),
                datetime: row.get("datettime"),
            })
            .collect();

        Ok(Response::new(TestDataList { data }))
    }
}
