use std::{collections::HashMap, path::Path, sync::{Arc, Mutex}, time::SystemTime};

use dashmap::DashMap;
use http::shared::{HttpSocket, LibError};
use serde::Deserialize;

use crate::{AorB, DynHttpSocket, arguments::Cli, elog_with_level, handlers::{HttpHandler, sanitize_path}, logger::{log_client_simple, loglevels}, servers::GenAddr, settings::Settings};
use owo_colors::OwoColorize;


#[derive(Debug, Deserialize)]
pub struct RouteConfig {
    #[serde(alias = "match-type")]
    pub match_type: String,
    #[serde(alias = "dir")]
    pub directory: String,
    pub router: Option<String>,
    pub auth: Option<String>,
    pub builtin: Option<String>,
    #[serde(alias = "404")]
    pub e404_file: Option<String>,
    #[serde(alias = "409")]
    pub e409_file: Option<String>,
}

pub struct SamicppHandler {
    pub _args: Arc<Cli>,
    pub settings: Arc<Settings>,

    pub config_modified: Mutex<SystemTime>,
    pub config: Mutex<DashMap<String, RouteConfig>>,
}
#[async_trait::async_trait]
impl HttpHandler for SamicppHandler {
    async fn entry(self: Arc<Self>, mut http: DynHttpSocket, addr: GenAddr) -> Result<(), LibError> {
        let client = http.read_until_head_complete().await?;
        log_client_simple(client);
        let path = sanitize_path(&client.path);
        let path_str = path.as_os_str().to_string_lossy();
        
        match self.update_config().await {
            Err(AorB::A(err)) => elog_with_level!(loglevels::SAMICPP_ROUTES_ERROR, "config I/O err {}", err.red()),
            Err(AorB::B(err)) => elog_with_level!(loglevels::SAMICPP_ROUTES_ERROR, "config json err {}", err.red()),
            _ => {}
        }

        Ok(())
    }
}
impl SamicppHandler {
    pub fn new(args: Arc<Cli>, settings: Arc<Settings>) -> Self {
        Self { 
            _args: args, 
            settings, 
            config_modified: Mutex::new(SystemTime::UNIX_EPOCH),
            config: Mutex::new(DashMap::new()),
        }
    }

    async fn update_config(&self) -> Result<bool, AorB<std::io::Error, serde_json::Error>> {
        let routes = Path::new(&self.settings.content.serve_dir).join(self.settings.content.routes_name.as_deref().unwrap_or("routes.json"));
        if let Ok(meta) = routes.metadata() {
            let modified = meta.modified().map_err(|e| AorB::A(e))?;
            if *self.config_modified.lock().unwrap() < modified {
                let file = tokio::fs::read(&routes).await.map_err(|e| AorB::A(e))?;
                *self.config.lock().unwrap() = serde_json::de::from_slice(&file).map_err(|e| AorB::B(e))?;
                Ok(true)
            }
            else {
                Ok(false)
            }
        }
        else {
            Ok(false)
        }
    }
}