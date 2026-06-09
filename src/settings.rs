use serde::Deserialize;


#[derive(Debug, Deserialize, Default)]
pub struct Settings {
    pub network: NetworkSettings,
    pub environment: EnvironmentSettings,
    pub content: ContentSettings,
    pub logging: LogSettings,
    // pub system: SystemSettings,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum OneOrMany<T> {
    One(T),
    Many(Vec<T>),
}
impl<T: Default> Default for OneOrMany<T> {
    fn default() -> Self {
        Self::One(T::default())
    }
}
impl<T> OneOrMany<T> {
    pub fn get(&self) -> &[T] {
        match self {
            Self::One(t) => std::slice::from_ref(t),
            Self::Many(v) => v,
        }
    }
    pub fn _convert(self) -> Vec<T> {
        match self {
            Self::One(t) => vec![t],
            Self::Many(v) => v,
        }
    }
}

fn def_one_or_many<T>() -> OneOrMany<T> { OneOrMany::Many(vec![]) }

#[derive(Debug, Deserialize, Default)]
pub struct SniConfig {
    pub domain: String,
    pub cert: String,
    pub key: String,
}

#[inline] pub const fn def_true() -> bool { true }

#[derive(Debug, Deserialize, Default)]
pub struct Binding {
    pub address: String,
    pub backlog: Option<u32>,
    pub reuse_addr: Option<bool>,
    pub reuse_port: Option<bool>,
    pub nodelay: Option<bool>,
    // pub dualstack: Option<bool>,
    pub recv_bufsize: Option<u32>,
    pub send_bufsize: Option<u32>,
    // pub max_connections: Option<u64>,

    // #[serde(default = "def_true")]
    // pub use_main_tls: bool,

    // #[serde(default)]
    // pub sni: Vec<SniConfig>,
    // pub default_key: Option<String>,
    // pub default_cert: Option<String>,
    // pub alpn: Option<OneOrMany<String>>,
}

#[derive(Debug, Deserialize, Default)]
pub struct NetworkSettings {
    #[serde(default)]
    pub binding: Vec<Binding>,
    
    #[serde(default = "def_one_or_many")]
    pub address: OneOrMany<String>,
    pub backlog: Option<u32>,
    
    #[serde(default)]
    pub sni: Vec<SniConfig>,
    pub default_key: Option<String>,
    pub default_cert: Option<String>,
    pub alpn: Option<OneOrMany<String>>,
}



#[derive(Debug, Deserialize, Default)]
pub struct EnvironmentSettings {
    pub cwd: Option<String>,

    #[serde(default = "def_true")]
    pub multi_threaded: bool,
    pub worker_threads: Option<usize>,
    pub thread_name: Option<String>,
    pub event_interval: Option<u32>,
    pub max_io_events_per_tick: Option<usize>,
    pub global_queue_interval: Option<u32>,
    pub thread_keep_alive_ns: Option<u64>,
    pub thread_stack_size: Option<usize>,
    pub max_blocking_threads: Option<usize>,
}

#[inline] fn def_serve_dir() -> String { "./".into() }
#[inline] const fn def_max_file_read_size() -> usize { 128 * 1024 }
#[inline] const fn def_file_chunk_size() -> usize { 128 * 1024 }

#[derive(Debug, Deserialize, Default)]
pub struct ContentSettings {
    pub handler: Option<String>,

    // general
    pub max_file_size: Option<usize>,
    #[serde(default = "def_serve_dir")]
    pub serve_dir: String,

    // samicpp
    pub routes_name: Option<String>,
    #[serde(default = "def_max_file_read_size")]
    pub max_file_read_size: usize,
    #[serde(default = "def_file_chunk_size")]
    pub file_chunk_size: usize,
}

#[inline] pub const fn def_false() -> bool { false }

// TODO: allow setting these with a loglevel
#[derive(Debug, Deserialize, Default)]
pub struct LogSettings {
    pub loglevel: Option<i16>,
    pub loglevel_template: Option<String>,
    #[serde(default = "def_false")]
    pub enable_unset: bool,
    #[serde(default = "def_false")]
    pub disable_unset: bool,


    pub init_error: Option<bool>,
    pub exit: Option<bool>,

    pub client_dump: Option<bool>,
    pub ip_dump: Option<bool>,

    pub request: Option<bool>,
    pub response: Option<bool>,
    pub response_time: Option<bool>,

    pub handler_error: Option<bool>,
    pub tls_upgrade_error: Option<bool>,
    pub content_handler_error: Option<bool>,

    pub http2_error: Option<bool>,
    pub http2_frame_dump: Option<bool>,

    pub routes_error: Option<bool>,
    pub routes_warning: Option<bool>,
    pub routes_update: Option<bool>,
    pub route_dump: Option<bool>,

    pub http_error: Option<bool>,
    pub http_error_detailed: Option<bool>,

    pub file_type_info: Option<bool>,
    pub file_processing_info: Option<bool>,
}
impl LogSettings {
    pub const fn default() -> Self {
        Self { 
            loglevel: None,
            loglevel_template: None,
            enable_unset: false,
            disable_unset: false,


            init_error: None,
            exit: None,

            client_dump: None,
            ip_dump: None,

            request: None,
            response: None,
            response_time: None,

            handler_error: None,
            tls_upgrade_error: None,
            content_handler_error: None,

            http2_error: None,
            http2_frame_dump: None,

            routes_error: None,
            routes_warning: None,
            routes_update: None,
            route_dump: None,

            http_error: None,
            http_error_detailed: None,
            
            file_type_info: None,
            file_processing_info: None,
        }
    }
    pub fn _disable_all(&mut self) {
        *self = Self { 
            loglevel: self.loglevel,
            loglevel_template: self.loglevel_template.clone(),
            enable_unset: self.enable_unset,
            disable_unset: self.disable_unset,


            init_error: Some(false),
            exit: Some(false),

            client_dump: Some(false),
            ip_dump: Some(false),

            request: Some(false),
            response: Some(false),
            response_time: Some(false),

            handler_error: Some(false),
            tls_upgrade_error: Some(false),
            content_handler_error: Some(false),

            http2_error: Some(false),
            http2_frame_dump: Some(false),

            routes_error: Some(false),
            routes_warning: Some(false),
            routes_update: Some(false),
            route_dump: Some(false),

            http_error: Some(false),
            http_error_detailed: Some(false),
            
            file_type_info: Some(false),
            file_processing_info: Some(false),
        }
    }
    pub fn _enable_all(&mut self) {
        *self = Self { 
            loglevel: self.loglevel,
            loglevel_template: self.loglevel_template.clone(),
            enable_unset: self.enable_unset,
            disable_unset: self.disable_unset,


            init_error: Some(true),
            exit: Some(true),

            client_dump: Some(true),
            ip_dump: Some(true),

            request: Some(true),
            response: Some(true),
            response_time: Some(true),

            handler_error: Some(true),
            tls_upgrade_error: Some(true),
            content_handler_error: Some(true),

            http2_error: Some(true),
            http2_frame_dump: Some(true),

            routes_error: Some(true),
            routes_warning: Some(true),
            routes_update: Some(true),
            route_dump: Some(true),

            http_error: Some(true),
            http_error_detailed: Some(true),
            
            file_type_info: Some(true),
            file_processing_info: Some(true),
        }

    }
    pub fn _unset_all(&mut self) {
        *self = Self { 
            loglevel: self.loglevel,
            loglevel_template: self.loglevel_template.clone(),
            enable_unset: self.enable_unset,
            disable_unset: self.disable_unset,

            ..Self::default()
        }
    }
    pub fn disable_unset(&mut self) {
        self.init_error.swap_if_none(false);
        self.exit.swap_if_none(false);

        self.client_dump.swap_if_none(false);
        self.ip_dump.swap_if_none(false);

        self.request.swap_if_none(false);
        self.response.swap_if_none(false);
        self.response_time.swap_if_none(false);

        self.handler_error.swap_if_none(false);
        self.tls_upgrade_error.swap_if_none(false);
        self.content_handler_error.swap_if_none(false);

        self.http2_error.swap_if_none(false);
        self.http2_frame_dump.swap_if_none(false);

        self.routes_error.swap_if_none(false);
        self.routes_warning.swap_if_none(false);
        self.routes_update.swap_if_none(false);
        self.route_dump.swap_if_none(false);

        self.http_error.swap_if_none(false);
        self.http_error_detailed.swap_if_none(false);

        self.file_type_info.swap_if_none(false);
        self.file_processing_info.swap_if_none(false);
    }
    pub fn enable_unset(&mut self) {
        self.init_error.swap_if_none(true);
        self.exit.swap_if_none(true);

        self.client_dump.swap_if_none(true);
        self.ip_dump.swap_if_none(true);

        self.request.swap_if_none(true);
        self.response.swap_if_none(true);
        self.response_time.swap_if_none(true);

        self.handler_error.swap_if_none(true);
        self.tls_upgrade_error.swap_if_none(true);
        self.content_handler_error.swap_if_none(true);

        self.http2_error.swap_if_none(true);
        self.http2_frame_dump.swap_if_none(true);

        self.routes_error.swap_if_none(true);
        self.routes_warning.swap_if_none(true);
        self.routes_update.swap_if_none(true);
        self.route_dump.swap_if_none(true);

        self.http_error.swap_if_none(true);
        self.http_error_detailed.swap_if_none(true);
        
        self.file_type_info.swap_if_none(true);
        self.file_processing_info.swap_if_none(true);
    }

    pub fn update_loglevel(&mut self, level: i16, restv: bool) {
        let mut rest = false;
        // debug
        if level & 1 != 0 {
            self.http2_error.swap_if_none(true);
            self.http2_frame_dump.swap_if_none(true);
            self.route_dump.swap_if_none(true);
            self.file_processing_info.swap_if_none(true);
            rest = restv;
        }
        // verbose
        if rest || level & 2 != 0 {
            self.ip_dump.swap_if_none(true);
            self.routes_update.swap_if_none(true);
            self.file_type_info.swap_if_none(true);
            self.http_error_detailed.swap_if_none(true);
            rest = restv;
        }
        // log
        if rest || level & 4 != 0 {
            self.response_time.swap_if_none(true);
            rest = restv;
        }
        // info
        if rest || level & 8 != 0 {
            self.exit.swap_if_none(true);
            self.request.swap_if_none(true);
            self.response.swap_if_none(true);
            rest = restv;
        }
        // warning
        if rest || level & 16 != 0 {
            self.routes_warning.swap_if_none(true);
            self.routes_error.swap_if_none(true);
            self.http_error.swap_if_none(true);
            rest = restv;
        }
        // error
        if rest || level & 32 != 0 {
            self.routes_error.swap_if_none(true);
            self.http_error.swap_if_none(true);
            rest = restv;
        }
        // critical error
        if rest || level & 64 != 0 {
            self.handler_error.swap_if_none(true);
            self.content_handler_error.swap_if_none(true);
            self.tls_upgrade_error.swap_if_none(true);
            rest = restv;
        }
        // fatal error
        if rest || level & 128 != 0 {
            self.init_error.swap_if_none(true);
        }
    }
    pub fn update_loglevel_template(&mut self, level: &str) {
        match level {
            "debug" => self.update_loglevel(1, true),
            "verbose" => self.update_loglevel(2, true),
            "log" => self.update_loglevel(4, true),
            "info" => self.update_loglevel(8, true),
            "warning" => self.update_loglevel(16, true),
            "error" => self.update_loglevel(32, true),
            "critical-error" => self.update_loglevel(64, true),
            "fatal-error" => self.update_loglevel(128, true),
            _ => ()
        }
    }
}


// #[derive(Debug, Deserialize, Default)]
// pub struct SystemSettings { }
trait SwapIfNone<T> {
    fn swap_if_none(&mut self, x: T) -> bool;
}
impl<T> SwapIfNone<T> for Option<T> {
    fn swap_if_none(&mut self, x: T) -> bool {
        if let None = *self {
            *self = Some(x);
            true
        }
        else {
            false
        }
    }
}