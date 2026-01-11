use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct IcLog {
    pub id: String,
    #[sqlx(rename = "type")]
    pub log_type: String,
    pub detail: Option<String>,
    pub date: NaiveDateTime,
    pub iid: Option<String>,
    pub machine_ip: String,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct IcLogWithDriver {
    pub id: String,
    #[sqlx(rename = "type")]
    pub log_type: String,
    pub detail: Option<String>,
    pub date: NaiveDateTime,
    pub iid: Option<String>,
    pub machine_ip: String,
    pub name: Option<String>,
}
