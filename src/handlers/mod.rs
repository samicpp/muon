#[cfg(debug_assertions)]
pub mod debug;
pub mod simple;

use std::sync::Arc;

use http::shared::LibError;

use crate::{DynHttpSocket, servers::GenAddr};


#[async_trait::async_trait]
pub trait HttpHandler{
    // fn new(args: Arc<Cli>, settings: Arc<Settings>) -> Self;
    async fn entry(self: Arc<Self>, http: DynHttpSocket, addr: GenAddr) -> Result<(), LibError>;
}