use std::{collections::HashMap, ops::Range, path::{Path, PathBuf}, ptr, sync::{Arc, RwLock}, time::SystemTime};

use dashmap::DashMap;
use libloading::Library;
use photon::{httprs_core::ffi::futures::FfiFuture, shared::{HttpSocket, LibError, LibResult}};
use regex::Regex;
use serde::Deserialize;
use tokio::{fs::File, io::{AsyncReadExt, AsyncSeekExt}};
use base64::{Engine, engine::general_purpose::STANDARD as b64std};

use crate::{AorB, DynHttpSocket, arguments::Cli, elog_with_level, handlers::{ClientInfo, HttpHandler, mime_types::MIME_TYPES, sanitize_path}, log_with_level, logger::log_client_simple, servers::GenAddr, settings::{OneOrMany, Settings, def_false}};
use owo_colors::OwoColorize;

pub mod builtin;
pub mod deno_scripting;
// pub mod force_symbol_exports;


#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct RouteConfig {
    pub match_type: MatchType,

    #[serde(skip)]
    pub regex: Option<Regex>,

    #[serde(alias = "dir")]
    pub directory: String,
    pub router: Option<String>,
    pub auth: Option<String>,
    pub middleware: Option<OneOrMany<String>>,

    #[serde(default = "def_false")]
    pub allow_invalid_clients: bool,
    pub forbid: Option<String>,
    pub forbid_end: Option<OneOrMany<String>>,
    pub forbid_start: Option<OneOrMany<String>>,
    #[serde(skip)]
    pub forbid_regex: Option<Regex>,

    pub e400_file: Option<String>,
    pub e403_file: Option<String>,
    pub e404_file: Option<String>,
    pub e409_file: Option<String>,
    pub e416_file: Option<String>,
    pub e500_file: Option<String>,
    pub e501_file: Option<String>,
}
impl Default for RouteConfig {
    fn default() -> Self {
        Self { 
            match_type: MatchType::Host, 

            regex: None,

            directory: ".".into(), 
            router: None, 
            auth: None, 
            middleware: None, 

            allow_invalid_clients: false,
            forbid: None, 
            forbid_end: None, 
            forbid_start: None, 
            forbid_regex: None, 

            e400_file: None, 
            e403_file: None, 
            e404_file: None, 
            e409_file: None, 
            e416_file: None, 
            e500_file: None, 
            e501_file: None, 
        }
    }
}
#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "kebab-case")]
pub enum MatchType {
    Always,
    Default,
    Host,
    Start,
    End,
    Regex,
    PathStart,
    Scheme,
    Protocol,
    Type,
    Domain,
}
pub struct FfiModule {
    #[allow(dead_code)]
    pub lib: Library,
    pub mtime: SystemTime,
    pub handle: unsafe extern "C" fn(*mut FfiFuture, *mut DynHttpSocket) -> (),
}
pub struct SamicppHandler {
    #[allow(dead_code)]
    pub args: Arc<Cli>,
    pub settings: Arc<Settings>,

    pub routes_modified: RwLock<SystemTime>,
    pub routes: RwLock<HashMap<String, Arc<RouteConfig>>>,
    pub routes_cache: DashMap<String, Arc<RouteConfig>>,
    pub routes_path: PathBuf,

    pub ffi_modules: DashMap<PathBuf, FfiModule>,
}


fn starts_with_case_insensitive<A: AsRef<str>, B: AsRef<str>>(haystack: A, needle: B) -> bool {
    let haystack = haystack.as_ref();
    let needle = needle.as_ref();

    if needle.len() <= haystack.len() {
        haystack.get(..needle.len()).map(|h: &str| h.eq_ignore_ascii_case(needle)).unwrap_or(false)
    }
    else {
        false
    }
}
fn ends_with_case_insensitive<A: AsRef<str>, B: AsRef<str>>(haystack: A, needle: B) -> bool {
    let haystack = haystack.as_ref();
    let needle = needle.as_ref();

    if needle.len() <= haystack.len() {
        haystack.get(haystack.len() - needle.len()..).map(|h: &str| h.eq_ignore_ascii_case(needle)).unwrap_or(false)
    }
    else {
        false
    }
}
fn domain_from_host(host: &str) -> &str {
    let portless = host.split(":").next().unwrap_or(host);
    
    psl::domain(portless.as_bytes()).map(|d| unsafe { str::from_utf8_unchecked(d.as_bytes()) }).unwrap_or(portless)
}
async fn authenticate(http: &mut DynHttpSocket, realm: &str, usrpass: &[u8]) -> LibResult<bool> {
    if let Some(authh) = http.get_client().headers.get("authorization")
    {
        let auth = authh[0].split(' ').last().unwrap_or(&authh[0]);
        if let Ok(auth) = b64std.decode(auth) && usrpass == auth {
            return Ok(true)
        }
    }
    http.set_status(401, "Unauthorized".to_owned());
    http.set_header("WWW-Authenticate", &format!("Basic realm=\"{realm}\", charset=\"UTF-8\""));
    http.set_header("Content-Length", "0");
    http.close(b"").await?;
    Ok(false)
}


#[async_trait::async_trait]
impl HttpHandler for SamicppHandler {
    async fn entry(self: Arc<Self>, mut http: DynHttpSocket, cinfo: ClientInfo) -> Result<(), LibError> {
        log_with_level!(false, self.settings.logging.ip_dump, "{}", &cinfo.addr);
        http.read_until_head_complete().await?;
        let client = http.get_client();
        let path = sanitize_path(&client.path);
        let path_str = path.as_os_str().to_string_lossy();
        let host = client.host.as_deref().unwrap_or("about:blank");
        let fullhost = format!("{}://{}{}", if cinfo.is_secure { "https" } else { "http" }, host, &path_str);
        let pfullhost = format!("[{}]{}", client.version, &fullhost);
        let domain = domain_from_host(host);

        log_with_level!(true, self.settings.logging.request, "\x1b[90m[{:?}]\x1b[0m {}", cinfo.addr, log_client_simple(client));
        
        match self.update_config().await {
            Err(AorB::A(err)) => elog_with_level!(true, self.settings.logging.routes_error, "routes I/O err {}", err.red()),
            Err(AorB::B(err)) => elog_with_level!(true, self.settings.logging.routes_error, "routes json err {}", err.red()),
            Ok(true) => log_with_level!(false, self.settings.logging.routes_update, "routes updated"),
            Ok(false) => {},
        }

        let mut route = None;
        let default = 
        if let Some(def) = self.routes_cache.get("default") { def.clone() }
        else {
            elog_with_level!(true, self.settings.logging.routes_warning, "no default entry in routes");
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
                    match opt.match_type {
                        MatchType::Always    => true,
                        MatchType::Default   => false,
                        MatchType::Host      => host.eq_ignore_ascii_case(label),
                        MatchType::Start     => starts_with_case_insensitive(&fullhost, label),
                        MatchType::End       => ends_with_case_insensitive(&fullhost, label),
                        MatchType::Regex     => opt.regex.as_ref().map(|r: &Regex| r.is_match(&fullhost)).unwrap_or(false),
                        MatchType::PathStart => starts_with_case_insensitive(&path_str, label),
                        MatchType::Scheme    => cinfo.is_secure && label.eq_ignore_ascii_case("https") || !cinfo.is_secure && label.eq_ignore_ascii_case("http"),
                        MatchType::Protocol  => client.version.to_string().eq_ignore_ascii_case(label),
                        MatchType::Type      => http.get_type().to_string().eq_ignore_ascii_case(label),
                        MatchType::Domain    => domain.eq_ignore_ascii_case(label),
                        // _                    => false,
                    }
                {
                    if self.settings.logging.route_dump.unwrap_or(false) {
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


        if !client.valid && !route.allow_invalid_clients {
            self.error(&mut http, &cinfo, &route, 400, &path, &path, "no invalid clients allowed", "detail").await?;
        }

        else if let Some(usrpass) = &route.auth && !authenticate(&mut http, &fullhost, usrpass.as_bytes()).await? {

        }

        else {
            if let Some(router) = route.router.as_deref() { 
                let router = Path::new(&self.settings.content.serve_dir).join(&route.directory).join(router);
                if !router.exists() {
                    self.error(&mut http, &cinfo, &route, 404, &path, &router, "router doesnt exist", "detail").await?;
                }
                else if !router.is_file() {
                    self.error(&mut http, &cinfo, &route, 501, &path, &router, "router is not a file", "detail").await?;
                }
                else {
                    self.file_handler(&mut http, &cinfo, &route, &path, &router, &fin_path).await?;
                }
            } else {
                self.dir_or_file(&mut http, &cinfo, &route, &path, &fin_path, &fin_path).await?;
            };
        }

        Ok(())
    }
}
impl SamicppHandler {
    pub fn new(args: Arc<Cli>, settings: Arc<Settings>) -> Self {
        let routes_path = Path::new(&settings.content.serve_dir).join(args.routes.as_deref().or(settings.content.routes_name.as_deref()).unwrap_or("routes.json"));

        Self { 
            args, 
            settings, 
            routes_modified: RwLock::new(SystemTime::UNIX_EPOCH),
            routes: RwLock::new(HashMap::new()),
            routes_cache: DashMap::new(),
            routes_path,
            ffi_modules: DashMap::new(),
        }
    }

    async fn update_config(&self) -> Result<bool, AorB<std::io::Error, serde_json::Error>> {
        // let routes = Path::new(&self.settings.content.serve_dir).join(self.args.routes.as_deref().or(self.settings.content.routes_name.as_deref()).unwrap_or("routes.json"));
        if let Ok(meta) = self.routes_path.metadata() {
            let modified = meta.modified().map_err(AorB::A)?;
            if *self.routes_modified.read().unwrap() < modified {
                let file = tokio::fs::read(&self.routes_path).await.map_err(AorB::A)?;
                
                let map: HashMap<String, RouteConfig> = serde_json::de::from_slice(&file).map_err(AorB::B)?;
                #[cfg(debug_assertions)] dbg!(&map);

                let mut nmap = HashMap::new();
                for (k, mut v) in map {
                    if v.match_type == MatchType::Regex {
                        v.regex = Regex::new(&k).ok();
                    }
                    if let Some(pat) = &v.forbid {
                        v.forbid_regex = Regex::new(pat).ok();
                    }
                    nmap.insert(k, Arc::new(v));
                }


                let mut omod: std::sync::RwLockWriteGuard<'_, SystemTime> = self.routes_modified.write().unwrap();
                let mut omap = self.routes.write().unwrap();
                
                self.routes_cache.clear();
                if let Some(def) = nmap.get("default") { self.routes_cache.insert("default".into(), def.clone()); }
                
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

    async fn error(&self, http: &mut DynHttpSocket, cinfo: &ClientInfo, conf: &RouteConfig, code: u16, path: &Path, target_path: &Path, reason: &str, detail: &str) -> LibResult<()> { 
        http.set_header("Content-Type", "text/plain");

        if self.settings.logging.http_error_detailed.unwrap_or(true) {
            println!("{path:?} {target_path:?} {code} {reason} {detail}");
        }

        match code {
            400 => {
                log_with_level!(true, self.settings.logging.http_error, "400 bad request");
                http.set_status(code, "Bad Request".into());

                if let Some(e400) = &conf.e400_file {
                    let e400_path = Path::new(&self.settings.content.serve_dir).join(&conf.directory).join(e400);
                    Box::pin(self.file_handler(http, cinfo, conf, path, &e400_path, target_path)).await?;
                }
                else {
                    http.close(b"broken request").await?;
                }
            }
            403 => {
                log_with_level!(true, self.settings.logging.http_error, "403 forbidden {target_path:?}");
                http.set_status(code, "Forbidden".into());
                
                if let Some(e403) = &conf.e403_file {
                    let e403_path = Path::new(&self.settings.content.serve_dir).join(&conf.directory).join(e403);
                    Box::pin(self.file_handler(http, cinfo, conf, path, &e403_path, target_path)).await?;
                }
                else { 
                    http.close(format!("forbidden").as_bytes()).await?; 
                }
            }
            404 => {
                log_with_level!(true, self.settings.logging.http_error, "404 not found {target_path:?}");
                http.set_status(code, "Not Found".into());
                
                if let Some(e404) = &conf.e404_file {
                    let e404_path = Path::new(&self.settings.content.serve_dir).join(&conf.directory).join(e404);
                    Box::pin(self.file_handler(http, cinfo, conf, path, &e404_path, target_path)).await?;
                }
                else { 
                    http.close(format!("couldnt find {path:?}").as_bytes()).await?; 
                }
            }
            409 => {
                log_with_level!(true, self.settings.logging.http_error, "409 conflict {target_path:?} {reason}");
                http.set_status(code, "Conflict".into());

                if let Some(e409) = &conf.e409_file {
                    let e409_path = Path::new(&self.settings.content.serve_dir).join(&conf.directory).join(e409);
                    Box::pin(self.file_handler(http, cinfo, conf, path, &e409_path, target_path)).await?;
                }
                else {
                    http.close(format!("something went wrong. {reason}").as_bytes()).await?;
                }
            }
            416 => {
                log_with_level!(true, self.settings.logging.http_error, "416 Range Not Satisfiable {target_path:?} {reason}");
                http.set_status(code, "Range Not Satisfiable".into());

                if let Some(e416 ) = &conf.e416_file {
                    let e416_path = Path::new(&self.settings.content.serve_dir).join(&conf.directory).join(e416);
                    Box::pin(self.file_handler(http, cinfo, conf, path, &e416_path, target_path)).await?;
                }
                else {
                    http.close(format!("Range Not Satisfiable. {reason}").as_bytes()).await?;
                }
            }

            500 => {
                log_with_level!(true, self.settings.logging.http_error, "500 internal server error");
                log_with_level!(true, self.settings.logging.http_error, "{}: {}", reason.red(), detail.red());
                http.set_status(code, "Internal Server Error".into());

                if let Some(e500) = &conf.e500_file {
                    let e500_path = Path::new(&self.settings.content.serve_dir).join(&conf.directory).join(e500);
                    Box::pin(self.file_handler(http, cinfo, conf, path, &e500_path, target_path)).await?;
                }
                else {
                    http.close(format!("something went wrong\r\n{reason}").as_bytes()).await?;
                }
            }
            501 => {
                log_with_level!(true, self.settings.logging.http_error, "501 unimplemented");
                http.set_status(code, "Not Implemented".into());

                if let Some(e501) = &conf.e501_file {
                    let e501_path = Path::new(&self.settings.content.serve_dir).join(&conf.directory).join(e501);
                    Box::pin(self.file_handler(http, cinfo, conf, path, &e501_path, target_path)).await?;
                }
                else {
                    http.close(b"not implemented").await?;
                }
            }

            _ => {
                log_with_level!(true, self.settings.logging.http_error, "{code} {reason}");
                http.set_status(code, "Error".into());
                http.close(format!("{reason} {detail}").as_bytes()).await?;
            }
        }

        Ok(())
    }

    #[inline]
    async fn dir_or_file(&self, http: &mut DynHttpSocket, cinfo: &ClientInfo, conf: &RouteConfig, path: &Path, file_path: &Path, real_path: &Path) -> LibResult<()> {
        if file_path.is_file() {
            self.file_handler(http, cinfo, conf, path, file_path, real_path).await
        }
        else if file_path.is_dir() {
            self.dir_handler(http, cinfo, conf, path, file_path, real_path).await
        }
        else if !file_path.exists() {
            self.error(http, cinfo, conf, 404, path, file_path, "doesnt exist", "detail").await
        }
        else {
            self.error(http, cinfo, conf, 501, path, file_path, "reason", "detail").await
        }
    }
    async fn dir_handler(&self, http: &mut DynHttpSocket, cinfo: &ClientInfo, conf: &RouteConfig, path: &Path, file_path: &Path, real_path: &Path) -> LibResult<()> { 
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
            self.file_handler(http, cinfo, conf, path, &found.path(), real_path).await
        } 
        else {
            self.error(http, cinfo, conf, 409, path, file_path, "no files found", "detail").await
        }
    }
    async fn file_handler(&self, http: &mut DynHttpSocket, cinfo: &ClientInfo, conf: &RouteConfig, path: &Path, file_path: &Path, real_path: &Path) -> LibResult<()> { 

        let meta = file_path.metadata()?;
        let name = file_path.file_name().map(|s| s.to_string_lossy()).unwrap_or("".into());
        let dots: Vec<&str> = name.split(".").collect();
        let last = *dots.last().unwrap_or(&"");
        let mime = *MIME_TYPES.get(last).unwrap_or(&"application/octet-stream");
        let mut file = File::open(file_path).await?;

        let forbidden = 
            conf.forbid_regex.as_ref().map(|r| r.is_match(&name)).unwrap_or(false) ||
            conf.forbid_end.as_ref().map(|vs| vs.get().iter().any(|v| name.ends_with(v))).unwrap_or(false) || 
            conf.forbid_start.as_ref().map(|vs| vs.get().iter().any(|v| name.ends_with(v))).unwrap_or(false);

        if forbidden {
            self.error(http, cinfo, conf, 403, path, file_path, "forbidden by config", "access to this file was denied").await?;
        }
        else if name.ends_with(".blank") {
            http.set_status(204, "No Content".into());
            http.close(b"").await?;
            log_with_level!(true, self.settings.logging.response, "204 No Content");
        }
        else if name.contains(".var.") || name.ends_with(".redirect") || name.ends_with(".link") {
            // TODO: constrain var files to the file size limits
            log_with_level!(false, self.settings.logging.file_type_info, "file is var, redirect, or link");
            http.set_header("Content-Type", mime);
            
            let mut content = String::new();
            file.read_to_string(&mut content).await?;

            for (h, v) in http.get_client().headers.iter() {
                let var = format!("%H-{}%", h.to_uppercase());
                let val = v[0].replace('%', "%PERCENT%");

                content = content.replace(&var, &val);
            }

            let user_agent = http.get_client().headers.get("user-agent").map(|v| v[0].as_str()).unwrap_or("");
            let user_agent_lower = user_agent.to_lowercase();
            let platform =
            if user_agent_lower.contains("windows") { "windows" }
            else if user_agent_lower.contains("android") { "android" }
            else if user_agent_lower.contains("macintosh") { "macos" }
            else if user_agent_lower.contains("iphone") { "iphone" }
            else if user_agent_lower.contains("ipad") { "ipad" }
            else if user_agent_lower.contains("linux") { "linux" }
            else if user_agent_lower.contains("curl") { "curl" }
            else { "unknown" };
            let host = http.get_client().host.as_deref().unwrap_or("about:blank");
            let domain = domain_from_host(host);

            match &cinfo.addr { 
                GenAddr::Net(net) => content = content.replace("%IP%", &net.ip().to_string()),
                GenAddr::Unix(unix) => {
                    if let Some(p) = unix.as_pathname() {
                        content = content.replace("%IP%", &p.to_string_lossy())
                    } else {
                        content = format!("{:?}", unix)
                    }
                },
            }
            content = content.replace("%FULL_IP%", &cinfo.addr.to_string());
            content = content.replace("%PATH%", &path.to_string_lossy().replace('%', "%PERCENT%"));
            content = content.replace("%HOST%", &http.get_client().host.as_deref().unwrap_or("about:blank").replace('%', "%PERCENT%"));
            content = content.replace("%SCHEME%", if cinfo.is_secure { "https" } else { "http" });
            content = content.replace("%BASE_DIR%", &self.settings.content.serve_dir);
            content = content.replace("%USER_AGENT%", &user_agent.replace('%', "%PERCENT%"));
            content = content.replace("%PLATFORM%", platform);
            content = content.replace("%DOMAIN%", domain);
            content = content.replace("%VERSION%", &http.get_client().version.to_string());
            
            content = content.replace("%PERCENT%", "%");

            if name.contains(".var.") {
                http.close(content.as_bytes()).await?;
                log_with_level!(true, self.settings.logging.response, "{:?} 200", file_path);
            } 
            else if name.ends_with(".redirect") {
                let location = content.replace("\n", "");
                
                let code =
                if dots.len() >= 3 {
                    let s = dots[dots.len() - 2];
                    s.parse().unwrap_or(302)
                }
                else {
                    302
                };

                http.set_status(code, "Found".to_owned());
                http.set_header("Location", location.trim());
                http.close(b"").await?;
                log_with_level!(true, self.settings.logging.response, "'{}' 302 Found", location);
            }
            else if name.ends_with(".link") {
                log_with_level!(false, self.settings.logging.file_processing_info, "passing link back into path handler");
                let link = Path::new(&content);
                Box::pin(self.dir_or_file(http, cinfo, conf, path, link, real_path)).await?;
            }
        }
        else if name.ends_with(".ffi.so") || name.ends_with(".ffi.dll") || name.ends_with(".ffi.dylib") {
            log_with_level!(false, self.settings.logging.file_type_info, "file is ffi module");
            let mut fut = FfiFuture::new(None, ptr::null_mut());

            unsafe {
                if let Some(lib) = self.ffi_modules.get(file_path) && lib.mtime >= meta.modified().unwrap_or(SystemTime::UNIX_EPOCH) {
                    let handle = &(*lib).handle;
                    handle((&mut fut) as *mut _, http as *mut _);
                    let _ = drop(lib);
                }
                else {
                    let lib = libloading::Library::new(file_path).map_err(|_| LibError::Io(std::io::Error::new(std::io::ErrorKind::Other, "couldnt open library")))?;
                    let init: libloading::Symbol<unsafe extern "C" fn() -> ()> = lib.get("init_muon_handler").map_err(|_| LibError::Io(std::io::Error::new(std::io::ErrorKind::Other, "couldnt find symbol init_muon_handler")))?;
                    let handle: libloading::Symbol<unsafe extern "C" fn(*mut FfiFuture, *mut DynHttpSocket) -> ()> = lib.get("muon_handler").map_err(|_| LibError::Io(std::io::Error::new(std::io::ErrorKind::Other, "couldnt find symbol init_muon_handler")))?;
                    let handle = *handle;
                    init();
                    handle((&mut fut) as *mut _, http as *mut _);
                    self.ffi_modules.insert(file_path.to_owned(), FfiModule { lib, mtime: meta.modified().unwrap_or(SystemTime::UNIX_EPOCH), handle });
                }
            }
            let _ = fut.await;
            log_with_level!(true, self.settings.logging.response, "{:?} 0 Done", file_path);
        }
        else {
            log_with_level!(false, self.settings.logging.file_type_info, "regular file");
            
            let name = 
            if name.ends_with(".gz") {
                http.set_header("Content-Encoding", "gzip");
                name.strip_suffix(".gz").unwrap_or(&name)
            }
            else if name.ends_with(".br") {
                http.set_header("Content-Encoding", "br");
                name.strip_suffix(".br").unwrap_or(&name)
            }
            else {
                &name
            };

            if name.ends_with(".download") {
                let name = name.strip_suffix(".download").unwrap_or(&name);
                http.set_header("Content-Disposition", &format!("attachment; filename={name}"));
            }

            let dots: Vec<&str> = name.split(".").collect();
            let last = *dots.last().unwrap_or(&"");
            let mime = *MIME_TYPES.get(last).unwrap_or(&"application/octet-stream");
            http.set_header("Content-Type", mime);

            http.set_header("Accept-Ranges", "bytes");
            // http.set_header("ETag", &format!("\"{}\"", meta.len(), meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok().map(|d| d.as_secs())).unwrap_or(0)));
            
            if name.ends_with(".gz") { http.set_header("Content-Encoding", "gzip"); }
            if name.ends_with(".br") { http.set_header("Content-Encoding", "br"); }

            let ranges =
            if let Some(rangehs) = http.get_client().headers.get("range") {
                let mut ranges: Vec<Range<u64>> = Vec::with_capacity(1);
                for rangeh in rangehs {
                    let Some(rangeh) = rangeh.strip_prefix("bytes=") else { continue };
                    for range in rangeh.split(",") {
                        let mut r = range.split("-");
                        let Some(start) = r.next() else { continue };
                        let Some(end) = r.next() else { continue };
                        
                        if start.is_empty() && end.is_empty() {
                            continue;
                        }
                        else if start.is_empty() && !end.is_empty() {
                            let Ok(end) = end.parse::<u64>() else { continue };
                            ranges.push(Range { 
                                start: meta.len() - end, 
                                end: meta.len() - 1 
                            });
                        }
                        else if !start.is_empty() && end.is_empty() {
                            let Ok(start) = start.parse() else { continue };
                            ranges.push(Range { 
                                start: start,
                                end: meta.len() - 1
                            });
                        }
                        else if !start.is_empty() && !end.is_empty() {
                            let Ok(start) = start.parse() else { continue };
                            let Ok(end) = end.parse() else { continue };
                            ranges.push(Range { 
                                start: start,
                                end: end,
                            });
                        }
                    }
                }
                Some(ranges)
            } else {
                None
            };

            if let Some(ranges) = &ranges && ranges.len() == 1 {
                log_with_level!(false, self.settings.logging.file_processing_info, "client requested range");

                let range = ranges[0].clone();
                http.set_status(206, "Partial Content".to_owned());

                http.set_header("Content-Range", &format!("bytes {}-{}/{}", range.start, range.end, meta.len()));
                
                if range.start > range.end || range.start > meta.len() || range.end > meta.len() {
                    self.error(http, cinfo, conf, 416, path, file_path, "invalid range", "detail").await?;
                }
                else {
                    let len = range.end - range.start;
                    file.seek(std::io::SeekFrom::Start(range.start)).await?;
                    http.set_header("Content-Length", &len.to_string());

                    if len < self.settings.content.max_file_read_size as u64 {
                        let mut out = vec![0u8; len as usize];
                        file.read_exact(&mut out).await?;
                        http.close(&out).await?;
                    }
                    else {
                        let mut chunk = vec![0u8; self.settings.content.file_chunk_size];
                        let count = len / self.settings.content.file_chunk_size as u64;
                        let remain = len % self.settings.content.file_chunk_size as u64;
                        
                        for _ in 0..count {
                            file.read_exact(&mut chunk).await?;
                            http.write(&chunk).await?;
                        }
                        
                        if count == 0 {
                            http.close(b"").await?;
                        } else {
                            let mut fin = vec![0u8; remain as usize];
                            file.read_exact(&mut fin).await?;
                            http.close(&fin).await?;
                        }
                    }

                    log_with_level!(true, self.settings.logging.response, "{:?} 200", file_path);
                }
            }
            else if let Some(ranges) = ranges {
                log_with_level!(false, self.settings.logging.file_processing_info, "client requested multiple ranges");

                let boundary = "aGV5IGhvdyBhcmUgeW91";
                http.set_header("Content-Type", &format!("multipart/byteranges; boundary={boundary}"));

                let mut errored = false;
                for range in ranges {
                    if range.start > range.end || range.start > meta.len() || range.end > meta.len() {
                        errored = true;
                        self.error(http, cinfo, conf, 416, path, file_path, "invalid range", "detail").await?;
                        break;
                    }
                    
                    let start = range.start;
                    let end = range.end;
                    let len = range.end - range.start;

                    file.seek(std::io::SeekFrom::Start(range.start)).await?;
                    http.write(format!("--{boundary}\r\nContent-Type: {mime}\r\nContent-Range: bytes {start}-{end}/{}\r\n\r\n", meta.len()).as_bytes()).await?;

                    if len < self.settings.content.max_file_read_size as u64 {
                        let mut out = vec![0u8; len as usize];
                        file.read_exact(&mut out).await?;
                        http.write(&out).await?;
                    }
                    else {
                        let mut chunk = vec![0u8; self.settings.content.file_chunk_size];
                        let count = len / self.settings.content.file_chunk_size as u64;
                        let remain = len % self.settings.content.file_chunk_size as u64;
                        
                        for _ in 0..count {
                            file.read_exact(&mut chunk).await?;
                            http.write(&chunk).await?;
                        }
                        
                        if count != 0 {
                            let mut fin = vec![0u8; remain as usize];
                            file.read_exact(&mut fin).await?;
                            http.write(&fin).await?;
                        }
                    }

                    http.write(b"\r\n").await?;
                }

                if !errored {
                    http.close(format!("--{boundary}--\r\n").as_bytes()).await?;

                    log_with_level!(true, self.settings.logging.response, "{:?} 200", file_path);
                }
            }
            else if meta.len() < self.settings.content.max_file_read_size as u64 {
                let mut out = vec![0u8; meta.len() as usize];
                file.read_exact(&mut out).await?;
                http.close(&out).await?;
                
                log_with_level!(true, self.settings.logging.response, "{:?} 200", file_path);
            }
            else {
                let mut chunk = vec![0u8; self.settings.content.file_chunk_size];
                let count = meta.len() / self.settings.content.file_chunk_size as u64;
                let remain = meta.len() % self.settings.content.file_chunk_size as u64;
                http.set_header("Content-Length", &meta.len().to_string());

                for _ in 0..count {
                    file.read_exact(&mut chunk).await?;
                    http.write(&chunk).await?;
                }
                
                if count == 0 {
                    http.close(b"").await?;
                } else {
                    let mut fin = vec![0u8; remain as usize];
                    file.read_exact(&mut fin).await?;
                    http.close(&fin).await?;
                }

                log_with_level!(true, self.settings.logging.response, "{:?} 200", file_path);
            }

        }

        Ok(())
    }
}