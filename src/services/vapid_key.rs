use crate::db::Database;
use crate::proto::timecard::{vapid_key_service_server::VapidKeyService, VapidKey};
use tonic::{Request, Response, Status};
use uuid::Uuid;

pub struct VapidKeyServiceImpl {
    db: Database,
}

impl VapidKeyServiceImpl {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

#[tonic::async_trait]
impl VapidKeyService for VapidKeyServiceImpl {
    async fn generate(
        &self,
        _request: Request<()>,
    ) -> Result<Response<VapidKey>, Status> {
        // 注: 本番環境では web-push クレートを使用してVAPIDキーを生成
        // ここでは簡略化のためダミーキーを生成
        let uuid = Uuid::new_v4().to_string();

        // ダミーキー (実際にはECDSA P-256キーペアを生成する必要あり)
        let public_key = base64::Engine::encode(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD,
            format!("public_key_{}", &uuid).as_bytes(),
        );
        let private_key = base64::Engine::encode(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD,
            format!("private_key_{}", &uuid).as_bytes(),
        );

        // データベースに保存
        sqlx::query(
            "INSERT INTO vapidkey (publicKey, privateKey, uuid) VALUES (?, ?, ?)",
        )
        .bind(&public_key)
        .bind(&private_key)
        .bind(&uuid)
        .execute(self.db.pool())
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(VapidKey {
            public_key,
            private_key,
            uuid,
        }))
    }
}
