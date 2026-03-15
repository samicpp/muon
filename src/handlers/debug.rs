use std::pin::Pin;

use crate::handlers::HttpHandler;



pub struct DebugHandler;
impl HttpHandler for DebugHandler{
    fn entry(&self) -> Pin<Box<dyn Future<Output = ()>>> {
        Box::pin(async move {
            println!("craxy")
        })
    }
}
