#[cfg(debug_assertions)]
pub mod debug;

use std::sync::Arc;

use http::shared::LibError;

use crate::DynHttpSocket;


#[async_trait::async_trait]
pub trait HttpHandler{
    // fn new(args: Arc<Cli>, settings: Arc<Settings>) -> Self;
    async fn entry(self: Arc<Self>, http: DynHttpSocket) -> Result<(), LibError>;
}