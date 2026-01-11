use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub grpc_port: u16,
    pub log_level: String,
}

impl Config {
    pub fn from_env() -> Result<Self, env::VarError> {
        // Load .env file if exists
        dotenvy::dotenv().ok();

        let db_host = env::var("RDB_HOST").unwrap_or_else(|_| "localhost".to_string());
        let db_user = env::var("RDB_USER").unwrap_or_else(|_| "root".to_string());
        let db_password = env::var("RDB_PASSWORD").unwrap_or_else(|_| "".to_string());
        let db_name = env::var("RDB_NAME").unwrap_or_else(|_| "db".to_string());

        let database_url = format!(
            "mysql://{}:{}@{}:3306/{}",
            db_user, db_password, db_host, db_name
        );

        let grpc_port = env::var("GRPC_PORT")
            .unwrap_or_else(|_| "50051".to_string())
            .parse()
            .unwrap_or(50051);

        let log_level = env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string());

        Ok(Config {
            database_url,
            grpc_port,
            log_level,
        })
    }
}
