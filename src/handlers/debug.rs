use std::sync::Arc;

use http::shared::{HttpSocket, LibError};

use crate::{DynHttpSocket, handlers::HttpHandler};



pub struct DebugHandler;
#[async_trait::async_trait]
impl HttpHandler for DebugHandler{
    async fn entry(self: Arc<Self>, mut http: DynHttpSocket) -> Result<(), LibError> {
        println!("craxy");
        http.close(b"body").await?;
        Ok(())
    }
}
