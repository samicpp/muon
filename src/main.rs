mod arguments;
mod settings;
// mod ssltls;
mod handlers;
mod servers;
// mod stream;
mod logger;
mod console;

use std::{path::PathBuf, sync::Arc, time::Duration};

use clap::Parser;
use photon::{extra::PolyHttpSocket, ffihttp::DynStream, httprs_core::runtime::RT};
use tokio::io::{ReadHalf, WriteHalf};
// use owo_colors::OwoColorize;

use crate::{arguments::{Cli, Level}, console::{ConTxRx, ConsoleCommand}, servers::start_servers, settings::{LogSettings, Settings}};

// pub static PROVIDER: LazyLock<Arc<CryptoProvider>> = LazyLock::new(|| Arc::new(rustls::crypto::aws_lc_rs::default_provider()));
// pub static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

pub type DynHttpSocket = PolyHttpSocket<ReadHalf<DynStream>, WriteHalf<DynStream>>;

fn main() {
    let args = Cli::parse();
    let sname = args.settings_name.as_ref().map(|s| s.to_owned()).unwrap_or("settings.toml".to_owned());
    let spfallback = "./".to_owned() + sname.as_str();
    let mut initial_logging = LogSettings::default();
    
    if let Some(lvl) = &args.loglevel {
        match lvl {
            Level::Name(level) => initial_logging.update_loglevel_template(level),
            Level::Number(level) => initial_logging.update_loglevel(*level, false),
        }
    }

    if args.loud {
        initial_logging.enable_all();
    } else if args.silent {
        initial_logging.disable_all();
    }


    if 
    let Some(cwd) = &args.cwd && 
    let Err(err) = std::env::set_current_dir(&cwd) 
    {
        elog_with_level!(true, initial_logging.init_error, "couldnt set cwd \x1b[91m{}\x1b[0m", err);
    }

    #[cfg(feature = "aws-lc-rs")]
    rustls::crypto::aws_lc_rs::default_provider().install_default().expect("couldnt install aws-lc-rs as default provider");
    #[cfg(feature = "ring")]
    rustls::crypto::ring::default_provider().install_default().expect("couldnt install ring as default provider");
    #[cfg(not(any(feature = "ring", feature = "aws-lc-rs")))]
    compile_error!("need either \"ring\" or \"aws-lc-rs\"");



    let args = Arc::new(args);
    let mut sett = setup_settings(&args, &initial_logging, &sname, &spfallback);
    // let sett_rwl = Arc::new(RwLock::new(settings));
    let (tx, rx) = tokio::sync::broadcast::channel::<ConsoleCommand>(4);
    let txrx = ConTxRx(tx,rx);
    let mut running = true;

    while running {
        let settings = Arc::new(sett.clone());
        
        if let Some(mut jh) = process(args.clone(), settings.clone(), txrx.clone()) { 
            if settings.environment.console {
                let args = args.clone();
                let txrx = txrx.clone();

                RT.spawn(console::console(args, settings.clone(), txrx)).unwrap();
            }

            let args = args.clone();
            let settings = settings.clone();
            let mut txrx = txrx.clone();
            running = 
            RT.block_on(async move {
                let mut term_counter = 0;
                let mut restart = false;

                loop {
                    tokio::select! {
                        res = &mut jh => {
                            let _ = res.map_err(|e| elog_with_level!(true, settings.logging.init_error, "server crahsed \x1b[91m{}\x1b[0m", e));
                            restart = false;
                            break;
                        },
                        cmd = txrx.1.recv() => {
                            match cmd {
                                Ok(ConsoleCommand::Kill) => {
                                    jh.abort();
                                    break;
                                },
                                Ok(ConsoleCommand::Stop) => {
                                    let _ = jh.await.map_err(|e| elog_with_level!(true, settings.logging.init_error, "server crahsed \x1b[91m{}\x1b[0m", e));
                                    break;
                                },
                                Ok(ConsoleCommand::Shutdown) => {
                                    let _ = jh.await.map_err(|e| elog_with_level!(true, settings.logging.init_error, "server crahsed \x1b[91m{}\x1b[0m", e));
                                    break;
                                },
                                Ok(ConsoleCommand::Reload) => {
                                    let _ = jh.await.map_err(|e| elog_with_level!(true, settings.logging.init_error, "server crahsed \x1b[91m{}\x1b[0m", e));
                                    jh = RT.spawn(start_servers(args.clone(), settings.clone(), txrx.clone())).unwrap();
                                },
                                Ok(ConsoleCommand::Restart) => {
                                    let _ = jh.await.map_err(|e| elog_with_level!(true, settings.logging.init_error, "server crahsed \x1b[91m{}\x1b[0m", e));
                                    restart = true;
                                    break;
                                },
                                Err(_) => {
                                    // elog_with_level!(true, settings.logging.init_error, "command crashed", e)
                                },
                            }
                        },
                        _ = tokio::signal::ctrl_c() => {
                            match term_counter {
                                0 => {
                                    _ = txrx.0.send(ConsoleCommand::Stop);
                                    log_with_level!(true, settings.logging.termination, "stopping");
                                },
                                1 => {
                                    _ = txrx.0.send(ConsoleCommand::Kill);
                                    log_with_level!(true, settings.logging.termination, "killing servers");
                                },
                                _ => {
                                    log_with_level!(true, settings.logging.termination, "terminating process");
                                    std::process::exit(1);
                                },
                            }
                            term_counter += 1;
                            restart = false;
                        },
                        _ = async move {
                            #[cfg(unix)]
                            match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                                Ok(mut s) => s.recv().await,
                                Err(_) => std::future::pending::<Option<()>>().await
                            }
                            #[cfg(not(unix))]
                            std::future::pending::<Option<()>>().await
                        } => {
                            let _ = txrx.0.send(ConsoleCommand::Stop);
                            log_with_level!(true, settings.logging.termination, "stopping");
                            restart = false;
                        }
                    }
                }

                restart
            }).unwrap();
        }
        // if let Some(cjh) = cjh {
        //     let _ = cjh.join();
        // }

        if running {
            let settings: Settings = setup_settings(&args, &initial_logging, &sname, &spfallback);
            sett = settings;
        }
    }

    elog_with_level!(true, sett.logging.exit, "done, exiting")
}

fn setup_settings(args: &Cli, initial_logging: &LogSettings, sname: &str, spfallback: &str) -> Settings {
    let settings = 
    match 
    if let Some(spath) = &args.settings { Ok(PathBuf::from(spath)) } 
    else { std::env::current_exe().map(|p| p.parent().map(|p| p.join(sname)).unwrap_or(PathBuf::from(spfallback))) } 
    {
        Err(e) => {
            elog_with_level!(true, initial_logging.init_error, "couldnt get executable path \x1b[91m{}\x1b[0m", e);
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

        
        init: initial_logging.init.or(settings.logging.init),
        init_error: initial_logging.init_error.or(settings.logging.init_error),
        exit: initial_logging.exit.or(settings.logging.exit),

        sni_setup: initial_logging.sni_setup.or(settings.logging.sni_setup),

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

        prereq_found: initial_logging.prereq_found.or(settings.logging.prereq_found),
        prereq_failed: initial_logging.prereq_failed.or(settings.logging.prereq_failed),
        prereq_passed: initial_logging.prereq_passed.or(settings.logging.prereq_passed),

        console_repeat: initial_logging.console_repeat.or(settings.logging.console_repeat),
        console_operation: initial_logging.console_operation.or(settings.logging.console_operation),

        termination: initial_logging.termination.or(settings.logging.termination),
    };

    settings
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

fn process(args: Arc<Cli>, settings: Arc<Settings>, txrx: ConTxRx) -> Option<tokio::task::JoinHandle<()>> {
    #[cfg(debug_assertions)] dbg!(&args);
    #[cfg(debug_assertions)] dbg!(&settings);

    if 
    let Some(cwd) = &settings.environment.cwd && 
    let Err(err) = std::env::set_current_dir(&cwd) 
    {
        elog_with_level!(true, settings.logging.init_error, "couldnt set cwd \x1b[91m{}\x1b[0m", err);
    }

    if RT.isset() && !settings.environment.rebuild_on_restart {
        let handle = RT.spawn(start_servers(args, settings, txrx)).unwrap();
        Some(handle)
    }
    else if settings.environment.multi_threaded {
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
                RT.set(rt);
                RT.spawn(start_servers(args, settings, txrx))
            },
            Err(err) => {
                elog_with_level!(true, settings.logging.init_error, "failed to build runtime \x1b[91m{}\x1b[0m", err);
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
                RT.set(rt);
                RT.spawn(start_servers(args, settings, txrx))
            },
            Err(err) => {
                elog_with_level!(true, settings.logging.init_error, "failed to build runtime \x1b[91m{}\x1b[0m", err);
                None
            }
        }
    }
}
