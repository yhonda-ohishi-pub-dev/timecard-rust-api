// gRPC ClientService implementation
// Returns connected Socket.IO client information

use crate::client_state::ClientState;
use crate::proto::timecard::{
    client_service_server::ClientService, ClientList, ConnectedClient,
};
use tonic::{Request, Response, Status};

pub struct ClientServiceImpl {
    clients: ClientState,
}

impl ClientServiceImpl {
    pub fn new(clients: ClientState) -> Self {
        Self { clients }
    }
}

#[tonic::async_trait]
impl ClientService for ClientServiceImpl {
    async fn get_all(&self, _request: Request<()>) -> Result<Response<ClientList>, Status> {
        let clients: Vec<ConnectedClient> = self
            .clients
            .get_all_clients()
            .into_iter()
            .map(|c| ConnectedClient {
                socket_id: c.socket_id,
                ip_address: c.ip_address,
                connected_at: c.connected_at.to_rfc3339(),
                last_activity: c.last_activity.to_rfc3339(),
            })
            .collect();

        let total = clients.len() as i32;

        Ok(Response::new(ClientList { clients, total }))
    }
}
