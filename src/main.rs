mod config;
mod db;
mod models;
mod services;

use std::sync::Arc;

use config::Config;
use db::Database;
use services::{
    DriverServiceImpl, FingerLogServiceImpl, ICLogServiceImpl, ICNonRegServiceImpl,
    NotificationServiceImpl, PicDataServiceImpl, TestServiceImpl, TmpDataServiceImpl,
    VapidKeyServiceImpl,
};
use tokio::sync::broadcast;
use tonic::transport::Server;
use tonic_reflection::server::Builder as ReflectionBuilder;
use tower_http::cors::{Any, CorsLayer};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

// Proto生成コードをインクルード
pub mod proto {
    pub mod timecard {
        tonic::include_proto!("timecard");

        pub const FILE_DESCRIPTOR_SET: &[u8] =
            tonic::include_file_descriptor_set!("timecard_descriptor");
    }
}

use proto::timecard::{
    driver_service_server::DriverServiceServer,
    finger_log_service_server::FingerLogServiceServer,
    ic_log_service_server::IcLogServiceServer,
    ic_non_reg_service_server::IcNonRegServiceServer,
    notification_service_server::NotificationServiceServer,
    pic_data_service_server::PicDataServiceServer,
    test_service_server::TestServiceServer,
    tmp_data_service_server::TmpDataServiceServer,
    vapid_key_service_server::VapidKeyServiceServer,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ロギング初期化
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // 設定読み込み
    let config = Config::from_env()?;
    info!("Starting gRPC server on port {}", config.grpc_port);

    // データベース接続
    info!("Connecting to database...");
    let database = Database::connect(&config.database_url).await?;
    info!("Database connected successfully");

    // イベントブロードキャスト用チャンネル
    let (broadcaster, _) = broadcast::channel(1024);
    let broadcaster = Arc::new(broadcaster);

    // gRPC サービス初期化
    let driver_service = DriverServiceImpl::new(database.clone());
    let ic_log_service = ICLogServiceImpl::new(database.clone());
    let pic_data_service = PicDataServiceImpl::new(database.clone());
    let tmp_data_service = TmpDataServiceImpl::new(database.clone());
    let finger_log_service = FingerLogServiceImpl::new(database.clone());
    let ic_non_reg_service = ICNonRegServiceImpl::new(database.clone());
    let vapid_key_service = VapidKeyServiceImpl::new(database.clone());
    let notification_service = NotificationServiceImpl::new(database.clone(), broadcaster.clone());
    let test_service = TestServiceImpl::new(database.clone());

    // Reflection サービス
    let reflection_service = ReflectionBuilder::configure()
        .register_encoded_file_descriptor_set(proto::timecard::FILE_DESCRIPTOR_SET)
        .build_v1()?;

    // CORSレイヤー (gRPC-Web用)
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_headers(Any)
        .allow_methods(Any)
        .expose_headers(Any);

    // サーバーアドレス
    let addr = format!("0.0.0.0:{}", config.grpc_port).parse()?;

    info!("gRPC server listening on {}", addr);

    // gRPC-Web対応サーバー起動
    Server::builder()
        .accept_http1(true) // gRPC-Web用にHTTP/1.1を許可
        .layer(cors)
        .layer(tonic_web::GrpcWebLayer::new()) // gRPC-Webサポート
        .add_service(reflection_service)
        .add_service(DriverServiceServer::new(driver_service))
        .add_service(IcLogServiceServer::new(ic_log_service))
        .add_service(PicDataServiceServer::new(pic_data_service))
        .add_service(TmpDataServiceServer::new(tmp_data_service))
        .add_service(FingerLogServiceServer::new(finger_log_service))
        .add_service(IcNonRegServiceServer::new(ic_non_reg_service))
        .add_service(VapidKeyServiceServer::new(vapid_key_service))
        .add_service(NotificationServiceServer::new(notification_service))
        .add_service(TestServiceServer::new(test_service))
        .serve(addr)
        .await?;

    Ok(())
}
