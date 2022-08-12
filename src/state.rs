use bitcoincore_rpc::Client;
use std::sync::Arc;

#[derive(Clone)]
pub struct State {
    pub rpc_client: Option<Arc<Client>>,
}

impl State {
    pub fn new() -> Self {
        State { rpc_client: None }
    }
}
