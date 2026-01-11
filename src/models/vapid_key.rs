use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct VapidKey {
    #[sqlx(rename = "publicKey")]
    pub public_key: String,
    #[sqlx(rename = "privateKey")]
    pub private_key: String,
    pub uuid: String,
}
