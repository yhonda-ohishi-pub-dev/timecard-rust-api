use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct IcNonReg {
    pub id: String,
    pub datetime: NaiveDateTime,
    pub deleted: Option<i8>,
    pub registered_id: Option<i32>,
}
