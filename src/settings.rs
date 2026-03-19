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

const fn def_true() -> bool { true }

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

#[derive(Debug, Deserialize, Default)]
pub struct ContentSettings {
    pub handler: Option<String>,
    pub max_file_size: Option<usize>,
    #[serde(default = "def_serve_dir")]
    pub serve_dir: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct LogSettings {
    pub loglevel: Option<u64>,
    pub loglevel_template: Option<String>,

    pub init_error: Option<bool>,
    pub exit: Option<bool>,
    pub client_dump: Option<bool>,
    pub request: Option<bool>,
    pub response: Option<bool>,
    pub response_time: Option<bool>,
    pub handler_error: Option<bool>,
    pub tls_upgrade_error: Option<bool>,
    pub content_handler_error: Option<bool>,
    pub http2_error: Option<bool>,
    pub http2_frame_dump: Option<bool>,
}

// #[derive(Debug, Deserialize, Default)]
// pub struct SystemSettings { }
