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

#[derive(Debug, Deserialize, Default)]
pub struct SniConfig {
    pub domain: String,
    pub cert: String,
    pub key: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct NetworkSettings {
    pub address: OneOrMany<String>,
    
    #[serde(default)]
    pub sni: Vec<SniConfig>,
    pub default_key: Option<String>,
    pub default_cert: Option<String>,
    pub alpn: Option<OneOrMany<String>>,
}


const fn def_multi_threaded() -> bool { true }

#[derive(Debug, Deserialize, Default)]
pub struct EnvironmentSettings {
    pub cwd: Option<String>,

    #[serde(default = "def_multi_threaded")]
    pub multi_threaded: bool,
    pub worker_threads: Option<usize>,
    pub thread_name: Option<String>,
    pub event_interval: Option<u32>,
    pub max_io_events_per_tick: Option<usize>,
    pub global_queue_interval: Option<u32>,
    pub thread_keep_alive_ns: Option<u64>,
    pub thread_stack_size: Option<usize>,
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
