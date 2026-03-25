use std::sync::Arc;

use photon::shared::{HttpSocket, LibError};

use crate::{DynHttpSocket, handlers::HttpHandler, servers::GenAddr};



pub struct DebugHandler;
#[async_trait::async_trait]
impl HttpHandler for DebugHandler{
    async fn entry(self: Arc<Self>, mut http: DynHttpSocket, _a: GenAddr, _is_secure: bool) -> Result<(), LibError> {
        // println!("craxy");
        dbg!(http.read_until_complete().await?);
        http.close(b"body").await?;
        Ok(())
    }
}
