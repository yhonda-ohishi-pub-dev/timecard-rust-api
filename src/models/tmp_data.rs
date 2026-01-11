use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct TmpData {
    pub machine_ip: String,
    pub tmp: String,
    pub amb: String,
    pub dist: String,
    pub date: NaiveDateTime,
    pub id: i32,
}

/// 一時データ + 画像 + ドライバー名の結合結果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TmpDataWithPic {
    pub machine_ip: String,
    pub tmp: String,
    pub amb: String,
    pub dist: String,
    pub date: NaiveDateTime,
    pub driver_id: Option<i32>,
    pub driver_name: Option<String>,
    pub pic_data_1: Option<String>, // base64
    pub pic_data_2: Option<String>, // base64
}
