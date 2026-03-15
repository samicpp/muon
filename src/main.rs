mod arguments;
mod settings;
mod ssltls;
mod handlers;
mod servers;

use std::{path::PathBuf, sync::{Arc, OnceLock}, time::Duration};

use clap::Parser;

use crate::{arguments::Cli, servers::start_servers, settings::Settings};


pub static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();


fn main() {
    let args = Cli::parse();
    let sname = args.settings_name.as_ref().map(|s| s.to_owned()).unwrap_or("settings.toml".to_owned());
    let spfallback = "./".to_owned() + &sname;


    if 
    let Some(cwd) = &args.cwd && 
    let Err(err) = std::env::set_current_dir(&cwd) 
    {
        eprintln!("couldnt set cwd");
        eprintln!("{err}");
    }


    let settings = 
    match 
    if let Some(spath) = &args.settings { Ok(PathBuf::from(spath)) } 
    else { std::env::current_exe().map(|p| p.parent().map(|p| p.join(sname)).unwrap_or(PathBuf::from(&spfallback))) } 
    {
        Err(e) => {
            eprintln!("couldnt get executable path");
            eprintln!("{e}");
            Err(())
        },
        Ok(me) => match load_settings(&me.as_os_str().to_str().unwrap_or(&spfallback)) {
            Ok(sett) => Ok(sett),
            Err(AorB::A(err)) => Err(eprintln!("io error {err}")),
            Err(AorB::B(err)) => Err(eprintln!("toml error {err}")),
        }
    };
    let settings = settings.unwrap_or_default();


    let args = Arc::new(args);
    let settings = Arc::new(settings);
    
    if let Some(jh) = process(args, settings) { 
        match RT.get().unwrap().block_on(jh) {
            Ok(()) => (),
            Err(e) => {
                eprintln!("couldnt wait for server to finish");
                eprintln!("{e}");
            }
        }
    }
    println!("done, exiting")
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
        eprintln!("couldnt set cwd");
        eprintln!("{err}");
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
                eprintln!("failed to build runtime");
                eprintln!("{err}");
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
                eprintln!("failed to build runtime");
                eprintln!("{err}");
                None
            }
        }
    }
}