mod arguments;
mod settings;
// mod ssltls;
mod handlers;
mod servers;
// mod stream;
mod logger;

use std::{path::PathBuf, sync::Arc, time::Duration};

use clap::Parser;
use photon::{extra::PolyHttpSocket, ffihttp::DynStream, httprs_core::ffi::own::RT};
use tokio::io::{ReadHalf, WriteHalf};
use owo_colors::OwoColorize;

use crate::{arguments::{Cli, Level}, servers::start_servers, settings::{LogSettings, Settings}};

// pub static PROVIDER: LazyLock<Arc<CryptoProvider>> = LazyLock::new(|| Arc::new(rustls::crypto::aws_lc_rs::default_provider()));
// pub static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

pub type DynHttpSocket = PolyHttpSocket<ReadHalf<DynStream>, WriteHalf<DynStream>>;

fn main() {
    let args = Cli::parse();
    let sname = args.settings_name.as_ref().map(|s| s.to_owned()).unwrap_or("settings.toml".to_owned());
    let spfallback = "./".to_owned() + &sname;
    let mut initial_logging = LogSettings::default();
    
    if let Some(lvl) = &args.loglevel {
        match lvl {
            Level::Name(level) => initial_logging.update_loglevel_template(level),
            Level::Number(level) => initial_logging.update_loglevel(*level, false),
        }
    }


    if 
    let Some(cwd) = &args.cwd && 
    let Err(err) = std::env::set_current_dir(&cwd) 
    {
        elog_with_level!(true, initial_logging.init_error, "couldnt set cwd {}", err.red());
    }

    #[cfg(feature = "aws-lc-rs")]
    rustls::crypto::aws_lc_rs::default_provider().install_default().expect("couldnt install aws-lc-rs as default provider");
    #[cfg(feature = "ring")]
    rustls::crypto::ring::default_provider().install_default().expect("couldnt install ring as default provider");
    #[cfg(not(any(feature = "ring", feature = "aws-lc-rs")))]
    compile_error!("need either \"ring\" or \"aws-lc-rs\"");



    let settings = 
    match 
    if let Some(spath) = &args.settings { Ok(PathBuf::from(spath)) } 
    else { std::env::current_exe().map(|p| p.parent().map(|p| p.join(sname)).unwrap_or(PathBuf::from(&spfallback))) } 
    {
        Err(e) => {
            elog_with_level!(true, initial_logging.init_error, "couldnt get executable path {}", e.red());
            Err(())
        },
        Ok(me) => match load_settings(&me.as_os_str().to_str().unwrap_or(&spfallback)) {
            Ok(sett) => Ok(sett),
            Err(AorB::A(err)) => Err(elog_with_level!(true, initial_logging.init_error, "io error {err}")),
            Err(AorB::B(err)) => Err(elog_with_level!(true, initial_logging.init_error, "toml error {err}")),
        }
    };

    let mut settings = settings.unwrap_or_default();

    if settings.logging.enable_unset { settings.logging.enable_unset(); }
    if settings.logging.disable_unset { settings.logging.disable_unset(); }
    if let Some(level) = settings.logging.loglevel { settings.logging.update_loglevel(level, false); }
    if let Some(level) = settings.logging.loglevel_template.clone() { settings.logging.update_loglevel_template(&level); }

    settings.logging = LogSettings { 
        loglevel: settings.logging.loglevel,
        loglevel_template: settings.logging.loglevel_template.clone(),
        enable_unset: settings.logging.enable_unset,
        disable_unset: settings.logging.disable_unset,

        
        init_error: initial_logging.init_error.or(settings.logging.init_error),
        exit: initial_logging.exit.or(settings.logging.exit),

        ip_dump: initial_logging.ip_dump.or(settings.logging.ip_dump),
        client_dump: initial_logging.client_dump.or(settings.logging.client_dump),

        request: initial_logging.request.or(settings.logging.request),
        response: initial_logging.response.or(settings.logging.response),
        response_time: initial_logging.response_time.or(settings.logging.response_time),

        handler_error: initial_logging.handler_error.or(settings.logging.handler_error),
        tls_upgrade_error: initial_logging.tls_upgrade_error.or(settings.logging.tls_upgrade_error),
        content_handler_error: initial_logging.content_handler_error.or(settings.logging.content_handler_error),

        http2_error: initial_logging.http2_error.or(settings.logging.http2_error),
        http2_frame_dump: initial_logging.http2_frame_dump.or(settings.logging.http2_frame_dump),

        routes_error: initial_logging.routes_error.or(settings.logging.routes_error),
        routes_update: initial_logging.routes_update.or(settings.logging.routes_update),
        routes_warning: initial_logging.routes_warning.or(settings.logging.routes_warning),
        route_dump: initial_logging.route_dump.or(settings.logging.route_dump),

        http_error: initial_logging.http_error.or(settings.logging.http_error),
        http_error_detailed: initial_logging.http_error_detailed.or(settings.logging.http_error_detailed),

        file_type_info: initial_logging.file_type_info.or(settings.logging.file_type_info),
        file_processing_info: initial_logging.file_processing_info.or(settings.logging.file_processing_info),
    };

    // if 
        // args.loglevel.is_none() && 
        // let Some(mut lvl) = settings.logging.loglevel.or(settings.logging.loglevel_template.as_deref().map(
        //     |preset| match preset {
        //         "all" | "everything" | "*" => u64::MAX,
        //         "nececities" | "needed" | "-" => loglevels::INIT_ERROR | loglevels::REQUEST,
        //         "verbose" | "+" => loglevels::INIT_ERROR | loglevels::REQUEST | loglevels::EXIT | loglevels::RESPONSE | loglevels::CONTENT_HANDLER_ERROR | loglevels::ROUTES_ERROR,
        //         _ => get_loglevel(),
        //     }
        // )) {
        
        // match settings.logging.init_error { Some(true) => lvl |= loglevels::INIT_ERROR, Some(false) => lvl &= !loglevels::INIT_ERROR, None => {} }
        // match settings.logging.exit { Some(true) => lvl |= loglevels::EXIT, Some(false) => lvl &= !loglevels::EXIT, None => {} }
        // match settings.logging.client_dump { Some(true) => lvl |= loglevels::CLIENT_DUMP, Some(false) => lvl &= !loglevels::CLIENT_DUMP, None => {} }
        // match settings.logging.request { Some(true) => lvl |= loglevels::REQUEST, Some(false) => lvl &= !loglevels::REQUEST, None => {} }
        // match settings.logging.response { Some(true) => lvl |= loglevels::RESPONSE, Some(false) => lvl &= !loglevels::RESPONSE, None => {} }
        // match settings.logging.response_time { Some(true) => lvl |= loglevels::RESPONSE_TIME, Some(false) => lvl &= !loglevels::RESPONSE_TIME, None => {} }
        // match settings.logging.handler_error { Some(true) => lvl |= loglevels::HANDLER_ERROR, Some(false) => lvl &= !loglevels::HANDLER_ERROR, None => {} }
        // match settings.logging.tls_upgrade_error { Some(true) => lvl |= loglevels::TLS_UPGRADE_ERROR, Some(false) => lvl &= !loglevels::TLS_UPGRADE_ERROR, None => {} }
        // match settings.logging.content_handler_error { Some(true) => lvl |= loglevels::CONTENT_HANDLER_ERROR, Some(false) => lvl &= !loglevels::CONTENT_HANDLER_ERROR, None => {} }
        // match settings.logging.http2_error { Some(true) => lvl |= loglevels::HTTP2_ERROR, Some(false) => lvl &= !loglevels::HTTP2_ERROR, None => {} }
        // match settings.logging.http2_frame_dump { Some(true) => lvl |= loglevels::HTTP2_FRAME_DUMP, Some(false) => lvl &= !loglevels::HTTP2_FRAME_DUMP, None => {} }
        // match settings.logging.routes_error { Some(true) => lvl |= loglevels::ROUTES_ERROR, Some(false) => lvl &= !loglevels::ROUTES_ERROR, None => {} }
        // match settings.logging.routes_update { Some(true) => lvl |= loglevels::ROUTES_UPDATE, Some(false) => lvl &= !loglevels::ROUTES_UPDATE, None => {} }
        // match settings.logging.route_dump { Some(true) => lvl |= loglevels::ROUTE_DUMP, Some(false) => lvl &= !loglevels::ROUTE_DUMP, None => {} }

        // set_loglevel(lvl);
    // }



    let args = Arc::new(args);
    let settings = Arc::new(settings);
    let settings2 = settings.clone();
    
    if let Some(jh) = process(args, settings) { 
        match RT.get().unwrap().block_on(jh) {
            Ok(()) => (),
            Err(e) => {
                elog_with_level!(true, settings2.logging.init_error, "couldnt wait for server to finish {}", e.red());
            }
        }
    }

    elog_with_level!(true, settings2.logging.exit, "done, exiting")
}

fn load_settings(path: &str) -> Result<Settings, AorB<std::io::Error, toml::de::Error>> {
    let raw = std::fs::read_to_string(path).map_err(AorB::A)?;
    let settings = toml::from_str::<Settings>(&raw).map_err(AorB::B)?;
    Ok(settings)
}

enum AorB<A, B>{
    A(A),
    B(B),
}
impl<A: std::fmt::Debug, B: std::fmt::Debug> std::fmt::Debug for AorB<A, B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::A(a) => f.debug_tuple("A").field(a).finish(),
            Self::B(b) => f.debug_tuple("B").field(b).finish(),
        }
    }
}

fn process(args: Arc<Cli>, settings: Arc<Settings>) -> Option<tokio::task::JoinHandle<()>> {
    #[cfg(debug_assertions)] dbg!(&args);
    #[cfg(debug_assertions)] dbg!(&settings);

    if 
    let Some(cwd) = &settings.environment.cwd && 
    let Err(err) = std::env::set_current_dir(&cwd) 
    {
        elog_with_level!(true, settings.logging.init_error, "couldnt set cwd {}", err.red());
    }

    if settings.environment.multi_threaded {
        let mut rt = tokio::runtime::Builder::new_multi_thread();
        
        rt.enable_all();
        if let Some(t) = settings.environment.worker_threads { rt.worker_threads(t); }
        if let Some(n) = &settings.environment.thread_name { rt.thread_name(n); }
        if let Some(e) = settings.environment.event_interval { rt.event_interval(e); }
        if let Some(m) = settings.environment.max_io_events_per_tick { rt.max_io_events_per_tick(m); }
        if let Some(g) = settings.environment.global_queue_interval { rt.global_queue_interval(g); }
        if let Some(d) = settings.environment.thread_keep_alive_ns { rt.thread_keep_alive(Duration::from_nanos(d)); }
        if let Some(s) = settings.environment.thread_stack_size { rt.thread_stack_size(s); }
        if let Some(b) = settings.environment.max_blocking_threads { rt.max_blocking_threads(b); }

        match rt.build() {
            Ok(rt) => {
                RT.set(rt).unwrap();
                let handle = RT.get().unwrap().spawn(start_servers(args, settings));
                Some(handle)
            },
            Err(err) => {
                elog_with_level!(true, settings.logging.init_error, "failed to build runtime {}", err.red());
                None
            }
        }
    }
    else {
        let mut rt = tokio::runtime::Builder::new_current_thread();
        
        rt.enable_all();
        if let Some(n) = &settings.environment.thread_name { rt.thread_name(n); }
        if let Some(s) = settings.environment.thread_stack_size { rt.thread_stack_size(s); }

        match rt.build() {
            Ok(rt) => {
                RT.set(rt).unwrap();
                let handle = RT.get().unwrap().spawn(start_servers(args, settings));
                Some(handle)
            },
            Err(err) => {
                elog_with_level!(true, settings.logging.init_error, "failed to build runtime {}", err.red());
                None
            }
        }
    }
}