// #[cfg(debug_assertions)]
pub mod debug;
#[cfg(feature = "simple")]
pub mod simple;
#[cfg(feature = "samicpp")]
pub mod samicpp;
pub mod mime_types;

use std::{path::{Path, PathBuf}, sync::Arc};

use http::shared::LibError;

use crate::{DynHttpSocket, servers::GenAddr};


#[async_trait::async_trait]
pub trait HttpHandler: Send + Sync {
    // fn new(args: Arc<Cli>, settings: Arc<Settings>) -> Self;
    async fn entry(self: Arc<Self>, http: DynHttpSocket, addr: GenAddr, is_secure: bool) -> Result<(), LibError>;
}

pub fn sanitize_path(path: &str) -> PathBuf {
    let path = path.replace("\\", "/");
    let path = path.split(&[':', '?', '#']).next().unwrap_or(&path);

    let mut sanit = PathBuf::new();

    use std::path::Component::*;
    for comp in Path::new(&path).components() {
        match comp {
            Prefix(_) => {},
            RootDir => {},
            CurDir => {},
            ParentDir => { sanit.pop(); },
            Normal(dir) => sanit.push(dir),
        }
    }

    sanit
}