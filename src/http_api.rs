// HTTP REST API endpoints for Cloudflare Workers access

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

use crate::db::Database;

#[derive(Clone)]
pub struct AppState {
    pub db: Database,
}

// Response types
#[derive(Serialize)]
pub struct DriverResponse {
    pub id: i32,
    pub name: String,
}

#[derive(Serialize)]
pub struct PicTmpResponse {
    pub date: String,
    pub machine_ip: String,
    pub id: Option<i32>,
    pub name: Option<String>,
    pub detail: Option<String>,
    pub pic_data_1: Option<String>,
    pub pic_data_2: Option<String>,
}

#[derive(Serialize)]
pub struct IcNonRegResponse {
    pub id: String,
    pub datetime: String,
    pub registered_id: Option<i32>,
}

#[derive(Serialize)]
pub struct IcLogResponse {
    pub id: i32,
    pub ic_id: String,
    pub driver_id: Option<i32>,
    pub datetime: String,
}

// Query params
#[derive(Deserialize)]
pub struct PicTmpQuery {
    pub limit: Option<i32>,
    pub start: Option<String>,
}

#[derive(Deserialize)]
pub struct DriverIdQuery {
    pub driver_id: i32,
}

#[derive(Deserialize)]
pub struct RegisterIcRequest {
    pub ic_id: String,
    pub driver_id: i32,
}

#[derive(Serialize)]
pub struct RegisterDirectIcResponse {
    pub success: bool,
    pub message: String,
    pub ic_id: Option<String>,
    pub driver_id: Option<i32>,
    pub driver_name: Option<String>,
}

pub fn create_router(db: Database) -> Router {
    let state = AppState { db };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/api/drivers", get(get_drivers))
        .route("/api/driver/{driver_id}", get(get_driver_by_id))
        .route("/api/pic_tmp", get(get_pic_tmp))
        .route("/api/ic_non_reg", get(get_ic_non_reg))
        .route("/api/ic_non_reg/register", post(register_ic))
        .route("/api/ic/register_direct", post(register_direct_ic))
        .route("/api/ic_log", get(get_ic_log))
        .route("/health", get(health_check))
        .layer(cors)
        .with_state(Arc::new(state))
}

async fn health_check() -> &'static str {
    "OK"
}

async fn get_drivers(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<DriverResponse>>, StatusCode> {
    let rows = sqlx::query("SELECT id, name FROM drivers ORDER BY id")
        .fetch_all(state.db.pool())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let drivers: Vec<DriverResponse> = rows
        .iter()
        .map(|row| DriverResponse {
            id: row.get("id"),
            name: row.get("name"),
        })
        .collect();

    Ok(Json(drivers))
}

async fn get_driver_by_id(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(driver_id): axum::extract::Path<i32>,
) -> Result<Json<Vec<DriverResponse>>, StatusCode> {
    let row = sqlx::query("SELECT id, name FROM drivers WHERE id = ? LIMIT 1")
        .bind(driver_id)
        .fetch_optional(state.db.pool())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    match row {
        Some(row) => Ok(Json(vec![DriverResponse {
            id: row.get("id"),
            name: row.get("name"),
        }])),
        None => Ok(Json(vec![])),
    }
}

async fn get_pic_tmp(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PicTmpQuery>,
) -> Result<Json<Vec<PicTmpResponse>>, StatusCode> {
    let limit = params.limit.unwrap_or(30);

    // Use start date or default to 2 days ago
    let start_date = params.start.unwrap_or_else(|| {
        let two_days_ago = chrono::Local::now() - chrono::Duration::days(2);
        two_days_ago.format("%Y-%m-%d %H:%M:%S").to_string()
    });

    // Query combining tmp_data, pic_data, and driver info
    let rows = sqlx::query(
        r#"
        SELECT
            t.date,
            t.machine_ip,
            COALESCE(ii.emp_id, f.id) as driver_id,
            d.name as driver_name,
            CASE
                WHEN ii.emp_id IS NOT NULL THEN 'tmp inserted by ic'
                WHEN f.id IS NOT NULL THEN 'tmp inserted by fing'
                ELSE 'tmp inserted'
            END as detail,
            p1.pic_data as pic_data_1,
            p2.pic_data as pic_data_2
        FROM tmp_data t
        LEFT JOIN pic_data p1 ON t.machine_ip = p1.machine_ip AND t.date = p1.date AND p1.pic_type = 1
        LEFT JOIN pic_data p2 ON t.machine_ip = p2.machine_ip AND t.date = p2.date AND p2.pic_type = 2
        LEFT JOIN ic_log il ON t.machine_ip = il.machine_ip AND t.date = il.datetime
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
        ) ii ON il.id = ii.ic_id
        LEFT JOIN finger_log f ON t.machine_ip = f.machine_ip AND t.date = f.datetime
        LEFT JOIN drivers d ON COALESCE(ii.emp_id, f.id) = d.id
        WHERE t.date >= ?
        ORDER BY t.date DESC
        LIMIT ?
        "#,
    )
    .bind(&start_date)
    .bind(limit)
    .fetch_all(state.db.pool())
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let data: Vec<PicTmpResponse> = rows
        .iter()
        .map(|row| {
            let date: chrono::NaiveDateTime = row.get("date");
            PicTmpResponse {
                date: date.format("%Y-%m-%dT%H:%M:%S").to_string(),
                machine_ip: row.get("machine_ip"),
                id: row.try_get("driver_id").ok(),
                name: row.try_get("driver_name").ok(),
                detail: row.try_get("detail").ok(),
                pic_data_1: row.try_get("pic_data_1").ok(),
                pic_data_2: row.try_get("pic_data_2").ok(),
            }
        })
        .collect();

    Ok(Json(data))
}

async fn get_ic_non_reg(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<IcNonRegResponse>>, StatusCode> {
    let two_days_ago = chrono::Local::now() - chrono::Duration::days(2);
    let start_date = two_days_ago.format("%Y-%m-%d %H:%M:%S").to_string();

    let rows = sqlx::query(
        "SELECT id, datetime, registered_id FROM ic_non_reged WHERE datetime >= ? ORDER BY datetime DESC",
    )
    .bind(&start_date)
    .fetch_all(state.db.pool())
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let items: Vec<IcNonRegResponse> = rows
        .iter()
        .map(|row| {
            let datetime: chrono::NaiveDateTime = row.get("datetime");
            IcNonRegResponse {
                id: row.get("id"),
                datetime: datetime.format("%Y-%m-%dT%H:%M:%S").to_string(),
                registered_id: row.try_get("registered_id").ok(),
            }
        })
        .collect();

    Ok(Json(items))
}

async fn register_ic(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RegisterIcRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // Update ic_non_reged
    sqlx::query("UPDATE ic_non_reged SET deleted = 1, registered_id = ? WHERE id = ?")
        .bind(req.driver_id)
        .bind(&req.ic_id)
        .execute(state.db.pool())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Insert into ic_id
    sqlx::query("INSERT INTO ic_id (ic_id, emp_id, deleted, date) VALUES (?, ?, 0, NOW())")
        .bind(&req.ic_id)
        .bind(req.driver_id)
        .execute(state.db.pool())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({
        "success": true,
        "ic_id": req.ic_id,
        "driver_id": req.driver_id
    })))
}

async fn register_direct_ic(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RegisterIcRequest>,
) -> Result<Json<RegisterDirectIcResponse>, StatusCode> {
    // 1. Validate driver exists
    let driver_row = sqlx::query("SELECT id, name FROM drivers WHERE id = ?")
        .bind(req.driver_id)
        .fetch_optional(state.db.pool())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let driver = match driver_row {
        Some(row) => row,
        None => {
            return Ok(Json(RegisterDirectIcResponse {
                success: false,
                message: "ドライバーIDが見つかりません".to_string(),
                ic_id: None,
                driver_id: None,
                driver_name: None,
            }));
        }
    };

    let driver_name: String = driver.get("name");

    // 2. Check if IC is already registered in ic_id table
    let existing = sqlx::query("SELECT ic_id FROM ic_id WHERE ic_id = ? AND deleted = 0")
        .bind(&req.ic_id)
        .fetch_optional(state.db.pool())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if existing.is_some() {
        return Ok(Json(RegisterDirectIcResponse {
            success: false,
            message: "このICカードは既に登録されています".to_string(),
            ic_id: Some(req.ic_id),
            driver_id: None,
            driver_name: None,
        }));
    }

    // 3. Insert/Update ic_non_reged with registered_id
    // Python client will complete registration on next IC touch
    sqlx::query(
        r#"INSERT INTO ic_non_reged (id, registered_id, datetime, deleted)
           VALUES (?, ?, NOW() + INTERVAL 9 HOUR, 0)
           ON DUPLICATE KEY UPDATE
           registered_id = VALUES(registered_id),
           datetime = NOW() + INTERVAL 9 HOUR,
           deleted = 0"#
    )
    .bind(&req.ic_id)
    .bind(req.driver_id)
    .execute(state.db.pool())
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(RegisterDirectIcResponse {
        success: true,
        message: "ICカード登録予約完了。次回ICタッチ時に登録されます".to_string(),
        ic_id: Some(req.ic_id),
        driver_id: Some(req.driver_id),
        driver_name: Some(driver_name),
    }))
}

async fn get_ic_log(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<IcLogResponse>>, StatusCode> {
    let rows = sqlx::query(
        "SELECT id, ic_id, iid as driver_id, datetime FROM ic_log ORDER BY datetime DESC LIMIT 100",
    )
    .fetch_all(state.db.pool())
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let logs: Vec<IcLogResponse> = rows
        .iter()
        .map(|row| {
            let datetime: chrono::NaiveDateTime = row.get("datetime");
            IcLogResponse {
                id: row.get("id"),
                ic_id: row.get("ic_id"),
                driver_id: row.try_get("driver_id").ok(),
                datetime: datetime.format("%Y-%m-%dT%H:%M:%S").to_string(),
            }
        })
        .collect();

    Ok(Json(logs))
}
