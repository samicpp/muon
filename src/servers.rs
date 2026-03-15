use std::{net::SocketAddr, sync::Arc};

use tokio::net::{TcpListener, TcpStream};

use crate::{arguments::Cli, handlers::{HttpHandler, debug::DebugHandler}, settings::Settings};



pub async fn start_servers(args: Arc<Cli>, settings: Arc<Settings>) {
    let addresses = args.addresses.as_ref().map(|v| v.as_slice()).unwrap_or(&[]).iter().chain(settings.network.address.get().iter()).collect::<Vec<&String>>();

    let handler: Arc<dyn HttpHandler + Send + Sync + 'static> = 
    match settings.content.handler.as_str() {
        #[cfg(debug_assertions)]
        "debug" => Arc::new(DebugHandler),

        _ => {
            eprintln!("no handler named {} available", settings.content.handler.as_str());
            return
        }
    };

    let mut servers = Vec::with_capacity(addresses.len());
    let mut jhs = Vec::with_capacity(addresses.len());

    for addr in addresses {
        let pl = addr.splitn(1, "://").collect::<Vec<&str>>();
        
        if pl.len() != 2 {
            eprintln!("invalid address: \"{addr}\"");
            continue;
        }

        let prot = pl[0];
        let loc = pl[1];

        match prot {
            "tcp" | "http" => {
                match TcpListener::bind(loc).await {
                    Err(err) => eprintln!("{err}"),
                    Ok(listener) => {
                        let server = Arc::new(TcpServer {
                            allow_h1: true, 
                            allow_h2c: true, 
                            allow_h2: true,

                            listener,
                            handler: handler.clone(),
                        });
                        servers.push(server.clone());
                        jhs.push(tokio::spawn(server.run()));
                    }
                }
            },

            _ => eprintln!("invalid protocol \"{prot}\""),
        }
    }

    for jh in jhs {
        let _ = jh.await;
    }
}


pub struct TcpServer {
    allow_h1: bool, 
    allow_h2c: bool, 
    allow_h2: bool,

    listener: TcpListener,
    handler: Arc<dyn HttpHandler + Send + Sync + 'static>,
}
impl TcpServer {
    pub async fn run(self: Arc<Self>) {
        loop {
            let Ok((stream, addr)) = self.listener.accept().await else { continue; };
            let selfc = self.clone();
            tokio::spawn(async move {
                match selfc.handle(stream, addr).await {
                    Ok(()) => (),
                    Err(err) => eprintln!("{err}"),
                }
            });
        }
    }
    pub async fn handle(self: Arc<Self>, stream: TcpStream, addr: SocketAddr) -> std::io::Result<()> {
        if self.allow_h2 {
            let mut peek = [0; 24];
            stream.peek(&mut peek).await?;
            if peek == http::http2::PREFACE {
                
            }
        }
        else {

        }

        

        Ok(())
    }
}