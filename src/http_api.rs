// HTTP REST API endpoints - Health check only

use axum::{routing::get, Router};
use tower_http::cors::{Any, CorsLayer};

pub fn create_router() -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/health", get(health_check))
        .layer(cors)
}

async fn health_check() -> &'static str {
    "OK"
}
