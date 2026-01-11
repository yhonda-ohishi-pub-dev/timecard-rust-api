use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct PicData {
    pub date: NaiveDateTime,
    pub cam: i32,
    pub pic: Vec<u8>, // LONGBLOB
    pub detail: String,
    pub machine_ip: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PicDataBase64 {
    pub date: NaiveDateTime,
    pub cam: i32,
    pub pic_base64: String,
    pub detail: String,
    pub machine_ip: String,
}

impl From<PicData> for PicDataBase64 {
    fn from(pic: PicData) -> Self {
        use base64::Engine;
        PicDataBase64 {
            date: pic.date,
            cam: pic.cam,
            pic_base64: base64::engine::general_purpose::STANDARD.encode(&pic.pic),
            detail: pic.detail,
            machine_ip: pic.machine_ip,
        }
    }
}
