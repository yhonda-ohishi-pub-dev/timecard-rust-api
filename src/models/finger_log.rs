use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct FingerLog {
    pub date: NaiveDateTime,
    pub machine_ip: String,
    pub id: i32,
    pub message: String,
}
