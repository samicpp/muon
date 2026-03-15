#[cfg(debug_assertions)]
pub mod debug;

use std::pin::Pin;


pub trait HttpHandler{
    // fn new(args: Arc<Cli>, settings: Arc<Settings>) -> Self;
    fn entry(&self) -> Pin<Box<dyn Future<Output = ()>>>;
}