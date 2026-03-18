use std::{path::Path, sync::Arc};

use http::shared::{HttpSocket, LibError};

use crate::{DynHttpSocket, arguments::Cli, handlers::{HttpHandler, sanitize_path}, logger::log_client_simple, servers::GenAddr, settings::Settings};



pub struct SamicppHandler {
    pub _args: Arc<Cli>,
    pub settings: Arc<Settings>,
}
#[async_trait::async_trait]
impl HttpHandler for SamicppHandler {
    async fn entry(self: Arc<Self>, mut http: DynHttpSocket, addr: GenAddr) -> Result<(), LibError> {
        let client = http.read_until_head_complete().await?;
        log_client_simple(client);

        Ok(())
    }
}
impl SamicppHandler {
    pub fn new(args: Arc<Cli>, settings: Arc<Settings>) -> Self {
        Self { 
            _args: args, 
            settings 
        }
    }
}