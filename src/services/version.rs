// gRPC VersionService implementation
// Returns build version information from environment variables

use crate::proto::timecard::{version_service_server::VersionService, VersionInfo};
use tonic::{Request, Response, Status};

pub struct VersionServiceImpl;

impl VersionServiceImpl {
    pub fn new() -> Self {
        Self
    }
}

impl Default for VersionServiceImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[tonic::async_trait]
impl VersionService for VersionServiceImpl {
    async fn get_version(&self, _request: Request<()>) -> Result<Response<VersionInfo>, Status> {
        let git_commit_full =
            std::env::var("GIT_COMMIT").unwrap_or_else(|_| "unknown".to_string());
        let git_commit_short =
            std::env::var("GIT_COMMIT_SHORT").unwrap_or_else(|_| "unknown".to_string());
        let build_date = std::env::var("BUILD_DATE").unwrap_or_else(|_| "unknown".to_string());

        // Get package version from compile-time
        let rust_version = env!("CARGO_PKG_VERSION").to_string();

        Ok(Response::new(VersionInfo {
            git_commit: git_commit_short,
            git_commit_full,
            build_date,
            rust_version,
        }))
    }
}
