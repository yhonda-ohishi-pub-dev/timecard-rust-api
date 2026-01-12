// HTTP REST API endpoints

use axum::{
    extract::State,
    http::StatusCode,
    routing::get,
    Json, Router,
};
use chrono::{Duration, Local, NaiveDateTime};
use serde::Serialize;
use sqlx::Row;
use tower_http::cors::{Any, CorsLayer};

use crate::db::Database;

/// CakePHP互換のレスポンス形式
#[derive(Debug, Serialize)]
pub struct IcLogResponse {
    pub id: Option<String>,
    pub datetime: String,
    pub machine_ip: String,
}

#[derive(Debug, Serialize)]
pub struct FingerLogResponse {
    pub id: i32,
    pub datetime: String,
    pub machine_ip: String,
}

/// データベース付きルーターを作成
pub fn create_router_with_db(db: Database) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/health", get(health_check))
        .route("/api/ic_log", get(get_ic_log))
        .route("/api/finger_log", get(get_finger_log))
        .with_state(db)
        .layer(cors)
}

async fn health_check() -> &'static str {
    "OK"
}

/// /api/ic_log - CakePHP互換エンドポイント
/// Node.js APIと同じ形式でデータを返す（過去2日間）
async fn get_ic_log(
    State(db): State<Database>,
) -> Result<Json<Vec<IcLogResponse>>, (StatusCode, String)> {
    let two_days_ago = Local::now() - Duration::days(2);
    let start_date = two_days_ago.format("%Y-%m-%d %H:%M:%S").to_string();

    let rows = sqlx::query(
        "SELECT date, iid, machine_ip FROM ic_log WHERE date >= ? ORDER BY date ASC",
    )
    .bind(&start_date)
    .fetch_all(db.pool())
    .await
    .map_err(|e| {
        tracing::error!("Database error in get_ic_log: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", e))
    })?;

    let logs: Vec<IcLogResponse> = rows
        .iter()
        .map(|row| {
            let date: NaiveDateTime = row.get("date");
            IcLogResponse {
                id: row.get("iid"),
                datetime: date.format("%Y-%m-%d %H:%M:%S").to_string(),
                machine_ip: row.get("machine_ip"),
            }
        })
        .collect();

    Ok(Json(logs))
}

/// /api/finger_log - CakePHP互換エンドポイント
/// Node.js APIと同じ形式でデータを返す（過去2日間）
async fn get_finger_log(
    State(db): State<Database>,
) -> Result<Json<Vec<FingerLogResponse>>, (StatusCode, String)> {
    let two_days_ago = Local::now() - Duration::days(2);
    let start_date = two_days_ago.format("%Y-%m-%d %H:%M:%S").to_string();

    let rows = sqlx::query(
        "SELECT id, date, machine_ip FROM finger_log WHERE date >= ? ORDER BY date ASC",
    )
    .bind(&start_date)
    .fetch_all(db.pool())
    .await
    .map_err(|e| {
        tracing::error!("Database error in get_finger_log: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", e))
    })?;

    let logs: Vec<FingerLogResponse> = rows
        .iter()
        .map(|row| {
            let date: NaiveDateTime = row.get("date");
            FingerLogResponse {
                id: row.get("id"),
                datetime: date.format("%Y-%m-%d %H:%M:%S").to_string(),
                machine_ip: row.get("machine_ip"),
            }
        })
        .collect();

    Ok(Json(logs))
}
