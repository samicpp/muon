mod arguments;
mod settings;
// mod ssltls;
mod handlers;
mod servers;
// mod stream;
mod logger;

use std::{path::PathBuf, sync::Arc, time::Duration};

use clap::Parser;
use http::{extra::PolyHttpSocket, ffihttp::DynStream, httprs_core::ffi::own::RT};
use tokio::io::{ReadHalf, WriteHalf};
use owo_colors::OwoColorize;

use crate::{arguments::Cli, logger::{LOGLEVEL, loglevels}, servers::start_servers, settings::Settings};

// pub static PROVIDER: LazyLock<Arc<CryptoProvider>> = LazyLock::new(|| Arc::new(rustls::crypto::aws_lc_rs::default_provider()));
// pub static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

pub type DynHttpSocket = PolyHttpSocket<ReadHalf<DynStream>, WriteHalf<DynStream>>;

fn main() {
    let args = Cli::parse();
    let sname = args.settings_name.as_ref().map(|s| s.to_owned()).unwrap_or("settings.toml".to_owned());
    let spfallback = "./".to_owned() + &sname;

    if let Some(lvl) = args.loglevel {
        LOGLEVEL.swap(lvl, std::sync::atomic::Ordering::Relaxed);
    }


    if 
    let Some(cwd) = &args.cwd && 
    let Err(err) = std::env::set_current_dir(&cwd) 
    {
        elog_with_level!(loglevels::INIT_ERROR, "couldnt set cwd {}", err.red());
    }


    let settings = 
    match 
    if let Some(spath) = &args.settings { Ok(PathBuf::from(spath)) } 
    else { std::env::current_exe().map(|p| p.parent().map(|p| p.join(sname)).unwrap_or(PathBuf::from(&spfallback))) } 
    {
        Err(e) => {
            elog_with_level!(loglevels::INIT_ERROR, "couldnt get executable path {}", e.red());
            Err(())
        },
        Ok(me) => match load_settings(&me.as_os_str().to_str().unwrap_or(&spfallback)) {
            Ok(sett) => Ok(sett),
            Err(AorB::A(err)) => Err(elog_with_level!(loglevels::INIT_ERROR, "io error {err}")),
            Err(AorB::B(err)) => Err(elog_with_level!(loglevels::INIT_ERROR, "toml error {err}")),
        }
    };
    let settings = settings.unwrap_or_default();

    if 
        args.loglevel.is_none() && 
        let Some(mut lvl) = settings.logging.loglevel.or(settings.logging.loglevel_template.as_deref().map(
            |preset| match preset {
                "all" | "everything" | "*" => u64::MAX,
                "nececities" | "needed" | "-" => loglevels::INIT_ERROR | loglevels::REQUEST,
                "verbose" | "+" => loglevels::INIT_ERROR | loglevels::REQUEST | loglevels::EXIT | loglevels::RESPONSE | loglevels::CONTENT_HANDLER_ERROR,
                _ => LOGLEVEL.load(std::sync::atomic::Ordering::Relaxed),
            }
        )) {
        
        match settings.logging.init_error { Some(true) => lvl |= loglevels::INIT_ERROR, Some(false) => lvl &= !loglevels::INIT_ERROR, None => {} }
        match settings.logging.exit { Some(true) => lvl |= loglevels::EXIT, Some(false) => lvl &= !loglevels::EXIT, None => {} }
        match settings.logging.client_dump { Some(true) => lvl |= loglevels::CLIENT_DUMP, Some(false) => lvl &= !loglevels::CLIENT_DUMP, None => {} }
        match settings.logging.request { Some(true) => lvl |= loglevels::REQUEST, Some(false) => lvl &= !loglevels::REQUEST, None => {} }
        match settings.logging.response { Some(true) => lvl |= loglevels::RESPONSE, Some(false) => lvl &= !loglevels::RESPONSE, None => {} }
        match settings.logging.response_time { Some(true) => lvl |= loglevels::RESPONSE_TIME, Some(false) => lvl &= !loglevels::RESPONSE_TIME, None => {} }
        match settings.logging.handler_error { Some(true) => lvl |= loglevels::HANDLER_ERROR, Some(false) => lvl &= !loglevels::HANDLER_ERROR, None => {} }
        match settings.logging.tls_upgrade_error { Some(true) => lvl |= loglevels::TLS_UPGRADE_ERROR, Some(false) => lvl &= !loglevels::TLS_UPGRADE_ERROR, None => {} }
        match settings.logging.content_handler_error { Some(true) => lvl |= loglevels::CONTENT_HANDLER_ERROR, Some(false) => lvl &= !loglevels::CONTENT_HANDLER_ERROR, None => {} }
        match settings.logging.http2_error { Some(true) => lvl |= loglevels::HTTP2_ERROR, Some(false) => lvl &= !loglevels::HTTP2_ERROR, None => {} }
        match settings.logging.http2_frame_dump { Some(true) => lvl |= loglevels::HTTP2_FRAME_DUMP, Some(false) => lvl &= !loglevels::HTTP2_FRAME_DUMP, None => {} }

        LOGLEVEL.swap(lvl, std::sync::atomic::Ordering::Relaxed);
    }


    let args = Arc::new(args);
    let settings = Arc::new(settings);
    
    if let Some(jh) = process(args, settings) { 
        match RT.get().unwrap().block_on(jh) {
            Ok(()) => (),
            Err(e) => {
                elog_with_level!(loglevels::INIT_ERROR, "couldnt wait for server to finish {}", e.red());
            }
        }
    }

    elog_with_level!(loglevels::EXIT, "done, exiting")
}

fn load_settings(path: &str) -> Result<Settings, AorB<std::io::Error, toml::de::Error>> {
    let raw = std::fs::read_to_string(path).map_err(|e| AorB::A(e))?;
    let settings = toml::from_str::<Settings>(&raw).map_err(|e| AorB::B(e))?;
    Ok(settings)
}

enum AorB<A, B>{
    A(A),
    B(B),
}


fn process(args: Arc<Cli>, settings: Arc<Settings>) -> Option<tokio::task::JoinHandle<()>> {
    #[cfg(debug_assertions)] dbg!(&args);
    #[cfg(debug_assertions)] dbg!(&settings);

    if 
    let Some(cwd) = &settings.environment.cwd && 
    let Err(err) = std::env::set_current_dir(&cwd) 
    {
        elog_with_level!(loglevels::INIT_ERROR, "couldnt set cwd {}", err.red());
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

        match rt.build() {
            Ok(rt) => {
                RT.set(rt).unwrap();
                let handle = RT.get().unwrap().spawn(start_servers(args, settings));
                Some(handle)
            },
            Err(err) => {
                elog_with_level!(loglevels::INIT_ERROR, "failed to build runtime {}", err.red());
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
                elog_with_level!(loglevels::INIT_ERROR, "failed to build runtime {}", err.red());
                None
            }
        }
    }
}