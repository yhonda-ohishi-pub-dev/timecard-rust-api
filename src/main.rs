mod client_state;
mod config;
mod db;
mod http_api;
mod models;
mod services;
mod socketio_server;

use std::sync::Arc;

use client_state::ClientState;
use config::Config;
use db::Database;
use services::{
    ClientServiceImpl, DriverServiceImpl, FingerLogServiceImpl, ICLogServiceImpl,
    ICNonRegServiceImpl, NotificationServiceImpl, PicDataServiceImpl, TestServiceImpl,
    TmpDataServiceImpl, VapidKeyServiceImpl, VersionServiceImpl,
};
use tokio::sync::broadcast;
use tonic::transport::Server;
use tonic_reflection::server::Builder as ReflectionBuilder;
use tower_http::cors::{Any, CorsLayer};
use tracing::{info, Level};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt};

// Proto生成コードをインクルード
pub mod proto {
    pub mod timecard {
        tonic::include_proto!("timecard");

        pub const FILE_DESCRIPTOR_SET: &[u8] =
            tonic::include_file_descriptor_set!("timecard_descriptor");
    }
}

use proto::timecard::{
    client_service_server::ClientServiceServer, driver_service_server::DriverServiceServer,
    finger_log_service_server::FingerLogServiceServer, ic_log_service_server::IcLogServiceServer,
    ic_non_reg_service_server::IcNonRegServiceServer,
    notification_service_server::NotificationServiceServer,
    pic_data_service_server::PicDataServiceServer, test_service_server::TestServiceServer,
    tmp_data_service_server::TmpDataServiceServer, vapid_key_service_server::VapidKeyServiceServer,
    version_service_server::VersionServiceServer,
};

/// 古いログファイルを削除（7日以上前）
fn cleanup_old_logs(log_dir: &str, max_age_days: u64) {
    let log_path = std::path::Path::new(log_dir);
    if !log_path.exists() {
        return;
    }

    let max_age = std::time::Duration::from_secs(max_age_days * 24 * 60 * 60);
    let now = std::time::SystemTime::now();

    if let Ok(entries) = std::fs::read_dir(log_path) {
        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata() {
                if let Ok(modified) = metadata.modified() {
                    if let Ok(age) = now.duration_since(modified) {
                        if age > max_age {
                            let _ = std::fs::remove_file(entry.path());
                        }
                    }
                }
            }
        }
    }
}

/// 定期的にログをクリーンアップするバックグラウンドタスク
fn spawn_log_cleanup_task() {
    tokio::spawn(async {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(24 * 60 * 60)); // 24時間ごと
        loop {
            interval.tick().await;
            cleanup_old_logs("logs", 7);
            tracing::debug!("Log cleanup completed");
        }
    });
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 起動時に古いログを削除 + 定期クリーンアップ開始
    cleanup_old_logs("logs", 7);
    spawn_log_cleanup_task();

    // ロギング初期化（コンソール + ファイル）
    let file_appender = RollingFileAppender::new(Rotation::DAILY, "logs", "server.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(
            fmt::layer()
                .with_writer(std::io::stdout)
                .with_ansi(true),
        )
        .with(
            fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false),
        )
        .with(tracing_subscriber::filter::LevelFilter::from_level(Level::INFO))
        .init();

    // 設定読み込み
    let config = Config::from_env()?;
    info!("Starting gRPC server on port {}", config.grpc_port);

    // データベース接続
    info!("Connecting to database...");
    let database = Database::connect(&config.database_url).await?;
    info!("Database connected successfully");

    // クライアント接続状態管理
    let client_state = ClientState::new();
    info!("Client state initialized");

    // イベントブロードキャスト用チャンネル
    let (broadcaster, _) = broadcast::channel(1024);
    let broadcaster = Arc::new(broadcaster);


    // gRPC サービス初期化
    let client_service = ClientServiceImpl::new(client_state.clone());
    let driver_service = DriverServiceImpl::new(database.clone());
    let ic_log_service = ICLogServiceImpl::new(database.clone());
    let pic_data_service = PicDataServiceImpl::new(database.clone());
    let tmp_data_service = TmpDataServiceImpl::new(database.clone());
    let finger_log_service = FingerLogServiceImpl::new(database.clone());
    let ic_non_reg_service = ICNonRegServiceImpl::new(database.clone());
    let vapid_key_service = VapidKeyServiceImpl::new(database.clone());
    let notification_service = NotificationServiceImpl::new(database.clone(), broadcaster.clone());
    let test_service = TestServiceImpl::new(database.clone());
    let version_service = VersionServiceImpl::new();

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
    let grpc_addr = format!("0.0.0.0:{}", config.grpc_port).parse()?;
    let http_port = config.http_port.unwrap_or(8080);
    let http_addr = format!("0.0.0.0:{}", http_port);

    info!("gRPC server listening on {}", grpc_addr);
    info!("HTTP API server listening on {}", http_addr);

    // HTTP API サーバー (health check only)
    let http_router = http_api::create_router();
    let http_listener = tokio::net::TcpListener::bind(&http_addr).await?;

    // gRPC-Web対応サーバー
    let grpc_server = Server::builder()
        .accept_http1(true) // gRPC-Web用にHTTP/1.1を許可
        .layer(cors)
        .layer(tonic_web::GrpcWebLayer::new()) // gRPC-Webサポート
        .add_service(reflection_service)
        .add_service(ClientServiceServer::new(client_service))
        .add_service(DriverServiceServer::new(driver_service))
        .add_service(IcLogServiceServer::new(ic_log_service))
        .add_service(PicDataServiceServer::new(pic_data_service))
        .add_service(TmpDataServiceServer::new(tmp_data_service))
        .add_service(FingerLogServiceServer::new(finger_log_service))
        .add_service(IcNonRegServiceServer::new(ic_non_reg_service))
        .add_service(VapidKeyServiceServer::new(vapid_key_service))
        .add_service(NotificationServiceServer::new(notification_service))
        .add_service(TestServiceServer::new(test_service))
        .add_service(VersionServiceServer::new(version_service))
        .serve(grpc_addr);

    // Socket.IO サーバー起動（設定されている場合）
    let socketio_server = if let Some(port) = config.socketio_server_port {
        info!("Starting Socket.IO server on port {}", port);
        let (socketio_layer, _io) = socketio_server::setup_socketio(
            database.clone(),
            client_state.clone(),
            config.cf_broadcast_url.clone(),
        );

        let socketio_cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_headers(Any)
            .allow_methods(Any);

        let socketio_router = axum::Router::new()
            .route("/health", axum::routing::get(|| async { "OK" }))
            .layer(socketio_layer)
            .layer(socketio_cors);

        Some(start_socketio_server(
            port,
            socketio_router,
            config.tls_cert_path.clone(),
            config.tls_key_path.clone(),
        ))
    } else {
        info!("SOCKETIO_SERVER_PORT not set, running without Socket.IO server");
        None
    };

    // サーバーを並行して起動
    if let Some(socketio_fut) = socketio_server {
        tokio::select! {
            result = grpc_server => {
                if let Err(e) = result {
                    tracing::error!("gRPC server error: {}", e);
                }
            }
            result = axum::serve(http_listener, http_router) => {
                if let Err(e) = result {
                    tracing::error!("HTTP server error: {}", e);
                }
            }
            result = socketio_fut => {
                if let Err(e) = result {
                    tracing::error!("Socket.IO server error: {}", e);
                }
            }
        }
    } else {
        // Socket.IOサーバーなしで起動
        tokio::select! {
            result = grpc_server => {
                if let Err(e) = result {
                    tracing::error!("gRPC server error: {}", e);
                }
            }
            result = axum::serve(http_listener, http_router) => {
                if let Err(e) = result {
                    tracing::error!("HTTP server error: {}", e);
                }
            }
        }
    }

    Ok(())
}

/// Start Socket.IO server with optional HTTPS
async fn start_socketio_server(
    port: u16,
    router: axum::Router,
    tls_cert_path: Option<String>,
    tls_key_path: Option<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));

    match (tls_cert_path, tls_key_path) {
        (Some(cert_path), Some(key_path)) => {
            // HTTPS mode
            info!("Socket.IO server starting with HTTPS on port {}", port);
            let tls_config = axum_server::tls_rustls::RustlsConfig::from_pem_file(&cert_path, &key_path)
                .await
                .map_err(|e| format!("Failed to load TLS config: {}", e))?;

            axum_server::bind_rustls(addr, tls_config)
                .serve(router.into_make_service())
                .await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }
        _ => {
            // HTTP mode
            info!("Socket.IO server starting with HTTP on port {}", port);
            let listener = tokio::net::TcpListener::bind(addr).await?;
            axum::serve(listener, router)
                .await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }
    }
}
