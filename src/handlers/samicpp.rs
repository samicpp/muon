use std::{collections::{HashMap, hash_map}, path::Path, sync::{Arc, LazyLock, Mutex, RwLock}, time::SystemTime};

use dashmap::DashMap;
use http::shared::{HttpSocket, LibError, LibResult};
use regex::Regex;
use serde::Deserialize;

use crate::{AorB, DynHttpSocket, arguments::Cli, elog_with_level, handlers::{HttpHandler, sanitize_path}, log_with_level, logger::{check_loglevel, log_client_simple, loglevels}, servers::GenAddr, settings::Settings};
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
impl Default for RouteConfig {
    fn default() -> Self {
        Self { 
            match_type: String::new(), 
            directory: ".".into(), 
            router: None, 
            auth: None, 
            builtin: None, 
            e404_file: None, 
            e409_file: None, 
        }
    }
}
pub struct SamicppHandler {
    pub args: Arc<Cli>,
    pub settings: Arc<Settings>,

    pub routes_modified: RwLock<SystemTime>,
    pub routes: RwLock<HashMap<String, Arc<RouteConfig>>>,
    pub routes_cache: DashMap<String, Arc<RouteConfig>>,
}

pub static DOMAIN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"([a-z|0-9|\-]+\.)?([a-z|0-9|\-]+)(?=:|$)").expect("domain regex invalid"));

#[async_trait::async_trait]
impl HttpHandler for SamicppHandler {
    async fn entry(self: Arc<Self>, mut http: DynHttpSocket, addr: GenAddr, is_secure: bool) -> Result<(), LibError> {
        log_with_level!(loglevels::IP_DUMP, "{}", &addr);
        http.read_until_head_complete().await?;
        let client = http.get_client();
        log_client_simple(client);
        let path = sanitize_path(&client.path);
        let path_str = path.as_os_str().to_string_lossy();
        let host = client.host.as_deref().unwrap_or("about:blank");
        let fullhost = format!("{}://{}{}", if is_secure { "http" } else { "https" }, host, &path_str);
        let pfullhost = format!("[{}]{}", client.version, &fullhost);
        let domain = DOMAIN.find(host).map(|h| h.as_str()).unwrap_or(host);
        
        match self.update_config().await {
            Err(AorB::A(err)) => elog_with_level!(loglevels::ROUTES_ERROR, "config I/O err {}", err.red()),
            Err(AorB::B(err)) => elog_with_level!(loglevels::ROUTES_ERROR, "config json err {}", err.red()),
            Ok(true) => log_with_level!(loglevels::ROUTES_UPDATE, "routes updated"),
            Ok(false) => {},
        }

        let mut route = None;
        let default = 
        if let Some(def) = self.routes_cache.get("default") { def.clone() }
        else {
            elog_with_level!(loglevels::ROUTES_ERROR, "no default entry");
            Arc::new(RouteConfig::default())
        };
        if let Some(conf) = self.routes_cache.get(&pfullhost) {
            route = Some(conf.clone());
        }
        else {
            let routes = self.routes.read().unwrap();
            for (label, opt) in routes.iter() {
                let label = label.as_str();
                if 
                    (opt.match_type == "host"       && host.eq_ignore_ascii_case(label)                                                                            ) ||
                    (opt.match_type == "start"      && fullhost.get(..label.len()).map(|h: &str| h.eq_ignore_ascii_case(label)).unwrap_or(false)                   ) ||
                    (opt.match_type == "end"        && fullhost.get(fullhost.len() - label.len()..).map(|h: &str| h.eq_ignore_ascii_case(label)).unwrap_or(false)  ) ||
                    (opt.match_type == "regex"      && Regex::new(label).map(|r: Regex| r.is_match(&fullhost)).unwrap_or(false)                                    ) ||
                    (opt.match_type == "path-start" && path_str.get(..label.len()).map(|h: &str| h.eq_ignore_ascii_case(label)).unwrap_or(false)                   ) ||
                    (opt.match_type == "scheme"     && (is_secure && label.eq_ignore_ascii_case("https") || !is_secure && label.eq_ignore_ascii_case("http"))      ) ||
                    (opt.match_type == "protocol"   && client.version.to_string().eq_ignore_ascii_case(label)                                                      ) ||
                    (opt.match_type == "type"       && http.get_type().to_string().eq_ignore_ascii_case(label)                                                     ) ||
                    (opt.match_type == "domain"     && domain.eq_ignore_ascii_case(label)                                                                          )
                {
                    if check_loglevel(loglevels::ROUTE_DUMP) {
                        println!("{} {:#?}", label, opt);
                    }
                    route = Some(opt.clone());
                    break;
                }
            }
            drop(routes);

            if let Some(route) = &route {
                self.routes_cache.insert(pfullhost, route.clone());
            }
        }
        
        let route = route.unwrap_or(default);
        let fin_path = Path::new(&self.settings.content.serve_dir).join(&route.directory).join(&path);

        if let Some(router) = route.router.as_deref() { 
            let router = Path::new(&self.settings.content.serve_dir).join(&route.directory).join(router);
            if !router.exists() {
                self.error(&mut http, 404, &path, &router, "reason", "detail").await?;
            }
            else if !router.is_file() {
                self.error(&mut http, 501, &path, &router, "router is not a file", "detail").await?;
            }
            else {
                self.file_handler(&mut http, &route, &path, &router, &fin_path).await?;
            }
        } else {
            self.dir_or_file(&mut http, &route, &path, &fin_path, &fin_path).await?;
        };


        Ok(())
    }
}
impl SamicppHandler {
    pub fn new(args: Arc<Cli>, settings: Arc<Settings>) -> Self {
        Self { 
            args, 
            settings, 
            routes_modified: RwLock::new(SystemTime::UNIX_EPOCH),
            routes: RwLock::new(HashMap::new()),
            routes_cache: DashMap::new(),
        }
    }

    async fn update_config(&self) -> Result<bool, AorB<std::io::Error, serde_json::Error>> {
        let routes = Path::new(&self.settings.content.serve_dir).join(self.args.routes.as_deref().or(self.settings.content.routes_name.as_deref()).unwrap_or("routes.json"));
        if let Ok(meta) = routes.metadata() {
            let modified = meta.modified().map_err(|e| AorB::A(e))?;
            if *self.routes_modified.read().unwrap() < modified {
                let file = tokio::fs::read(&routes).await.map_err(|e| AorB::A(e))?;
                
                let map: HashMap<String, RouteConfig> = serde_json::de::from_slice(&file).map_err(|e| AorB::B(e))?;
                #[cfg(debug_assertions)] dbg!(&map);

                let mut nmap = HashMap::new();
                for (k, v) in map {
                    nmap.insert(k, Arc::new(v));
                }


                let mut omod = self.routes_modified.write().unwrap();
                let mut omap = self.routes.write().unwrap();
                
                if let Some(def) = nmap.get("default") { self.routes_cache.insert("default".into(), def.clone()); }
                self.routes_cache.clear();
                
                *omod = modified;
                *omap = nmap;


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

    async fn error(&self, http: &mut DynHttpSocket, code: u16, path: &Path, target_path: &Path, reason: &str, detail: &str) -> LibResult<()> { 
        http.set_header("Content-Type", "text/plain");

        if check_loglevel(loglevels::HTTP_ERRORS) {
            println!("{code} {reason}");
        }

        match code {
            400 => {
                log_with_level!(loglevels::HTTP_ERRORS, "400 bad request");
                http.set_status(code, "Bad Request".into());
                http.close(b"broken request").await?;
            }
            404 => {
                log_with_level!(loglevels::HTTP_ERRORS, "404 not found {target_path:?}");
                http.set_status(code, "Not Found".into());
                http.close(format!("couldnt find {path:?}").as_bytes()).await?;
            }
            409 => {
                log_with_level!(loglevels::HTTP_ERRORS, "409 conflict {target_path:?} {reason}");
                http.set_status(code, "Conflict".into());
                http.close(format!("something went wrong. {reason}").as_bytes()).await?;
            }

            500 => {
                log_with_level!(loglevels::HTTP_ERRORS, "500 internal server error");
                log_with_level!(loglevels::HTTP_ERRORS, "{}: {}", reason.red(), detail.red());
                http.set_status(code, "Internal Server Error".into());
                http.close(format!("something went wrong\r\n{reason}").as_bytes()).await?;
            }
            501 => {
                log_with_level!(loglevels::HTTP_ERRORS, "501 unimplemented");
                http.set_status(code, "Not Implemented".into());
                http.close(b"").await?;
            }

            _ => {
                log_with_level!(loglevels::HTTP_ERRORS, "{code} {reason}");
                http.set_status(code, "Error".into());
                http.close(format!("{reason} {detail}").as_bytes()).await?;
            }
        }

        Ok(())
    }

    #[inline]
    async fn dir_or_file(&self, http: &mut DynHttpSocket, conf: &RouteConfig, path: &Path, file_path: &Path, real_path: &Path) -> LibResult<()> {
        if file_path.is_file() {
            self.file_handler(http, conf, path, file_path, real_path).await
        }
        else if file_path.is_dir() {
            self.dir_handler(http, conf, path, file_path, real_path).await
        }
        else {
            self.error(http, 501, path, file_path, "reason", "detail").await
        }
    }
    async fn dir_handler(&self, http: &mut DynHttpSocket, conf: &RouteConfig, path: &Path, file_path: &Path, real_path: &Path) -> LibResult<()> { 
        let name = file_path.file_name().map(|s| s.to_string_lossy()).unwrap_or("index".into());

        let mut found = None;
        let mut dir = tokio::fs::read_dir(&file_path).await?;
        while let Some(file) = dir.next_entry().await? {
            if 
                file.metadata().await.map(|m| m.is_file()).unwrap_or(false) && 
                (
                    file.file_name().to_string_lossy().starts_with(name.as_ref()) || 
                    file.file_name().to_string_lossy().starts_with("index")
                ) 
            {
                found = Some(file);
                break;
            }
        }

        if let Some(found) = found {
            self.file_handler(http, conf, path, &found.path(), real_path).await
        } 
        else {
            self.error(http, 409, path, file_path, "no files found", "detail").await
        }
    }
    async fn file_handler(&self, http: &mut DynHttpSocket, conf: &RouteConfig, path: &Path, file_path: &Path, real_path: &Path) -> LibResult<()> { 

        unimplemented!()
    }
}