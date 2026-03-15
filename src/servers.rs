use std::{net::SocketAddr, sync::Arc};

use http::{http1::server::Http1Socket, http2::{core::Http2Settings, server::Http2Socket, session::Http2Session}, shared::{HttpVersion, LibError}};
#[cfg(feature = "unix-sockets")]
use tokio::net::{UnixStream, UnixListener};
use tokio::net::{TcpListener, TcpStream};

use crate::{DynHttpSocket, arguments::Cli, handlers::{HttpHandler, debug::DebugHandler}, settings::Settings, stream::PolyStream};


pub static H2SETTINGS: Http2Settings = Http2Settings::default_no_push();


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

    // let mut servers = Vec::with_capacity(addresses.len());
    let mut jhs = Vec::with_capacity(addresses.len());

    for addr in addresses {
        let mut pl = addr.splitn(2, "://");

        let Some(prot) = pl.next() else { continue; };
        let Some(loc) = pl.next() else {
            eprintln!("invalid address: \"{addr}\"");
            continue;
        };

        match prot {
            "tcp" | "http" => {
                match TcpListener::bind(loc).await {
                    Err(err) => eprintln!("{err}"),
                    Ok(listener) => {
                        let server = Arc::new(TcpServer {
                            allow_h2c: true, 
                            allow_h2: true,
                            overide: None,

                            listener,
                            handler: handler.clone(),
                        });
                        // servers.push(Server::Tcp(server.clone()));
                        jhs.push(tokio::spawn(server.run()));
                    }
                }
            },
            "http1" => {
                match TcpListener::bind(loc).await {
                    Err(err) => eprintln!("{err}"),
                    Ok(listener) => {
                        let server = Arc::new(TcpServer {
                            allow_h2c: false, 
                            allow_h2: false,
                            overide: None,

                            listener,
                            handler: handler.clone(),
                        });
                        // servers.push(Server::Tcp(server.clone()));
                        jhs.push(tokio::spawn(server.run()));
                    }
                }
            },
            "http1.1" => {
                match TcpListener::bind(loc).await {
                    Err(err) => eprintln!("{err}"),
                    Ok(listener) => {
                        let server = Arc::new(TcpServer {
                            allow_h2c: false, 
                            allow_h2: false,
                            overide: Some(HttpVersion::Http11),

                            listener,
                            handler: handler.clone(),
                        });
                        // servers.push(Server::Tcp(server.clone()));
                        jhs.push(tokio::spawn(server.run()));
                    }
                }
            },
            "http1.0" => {
                match TcpListener::bind(loc).await {
                    Err(err) => eprintln!("{err}"),
                    Ok(listener) => {
                        let server = Arc::new(TcpServer {
                            allow_h2c: false, 
                            allow_h2: false,
                            overide: Some(HttpVersion::Http10),

                            listener,
                            handler: handler.clone(),
                        });
                        // servers.push(Server::Tcp(server.clone()));
                        jhs.push(tokio::spawn(server.run()));
                    }
                }
            },
            "http0.9" => {
                match TcpListener::bind(loc).await {
                    Err(err) => eprintln!("{err}"),
                    Ok(listener) => {
                        let server = Arc::new(TcpServer {
                            allow_h2c: false, 
                            allow_h2: false,
                            overide: Some(HttpVersion::Http09),

                            listener,
                            handler: handler.clone(),
                        });
                        // servers.push(Server::Tcp(server.clone()));
                        jhs.push(tokio::spawn(server.run()));
                    }
                }
            },

            "http2" => {
                match TcpListener::bind(loc).await {
                    Err(err) => eprintln!("{err}"),
                    Ok(listener) => {
                        let server = Arc::new(Http2Server {
                            listener,
                            handler: handler.clone(),
                        });
                        // servers.push(Server::TcpH2(server.clone()));
                        jhs.push(tokio::spawn(server.run()));
                    }
                }
            },

            #[cfg(feature = "unix-sockets")]
            "unix" => {
                match UnixListener::bind(loc) {
                    Err(err) => eprintln!("{err}"),
                    Ok(listener) => {
                        let server = Arc::new(UnixServer {
                            listener,
                            handler: handler.clone(),
                        });
                        // servers.push(Server::TcpH2(server.clone()));
                        jhs.push(tokio::spawn(server.run()));
                    }
                }
            }

            _ => eprintln!("invalid protocol \"{prot}\""),
        }
    }

    for jh in jhs {
        let _ = jh.await;
    }
}


// async fn http2_handler<R, W, F, Fut>(http2: Http2Session<R, W>, cb: F) -> std::io::Result<()>
// where 
//     R: ReadStream, 
//     W: WriteStream,
//     F: Fn(Http2Socket<R, W>) -> Fut,
//     Fut: Future<Output = ()> + Send + 'static,
// {
//     Ok(())
// }

// pub enum Server {
//     Tcp(Arc<TcpServer>),
//     TcpH2(Arc<Http2Server>),
//     Unix(Arc<UnixServer>),
// }
pub struct TcpServer {
    allow_h2c: bool,
    allow_h2: bool,
    overide: Option<HttpVersion>,

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
    pub async fn handle(self: Arc<Self>, stream: TcpStream, addr: SocketAddr) -> Result<(), LibError> {
        dbg!(&addr);
        let mut peek = [0; 24];
        stream.peek(&mut peek).await?;
        if self.allow_h2 && peek == http::http2::PREFACE{
            let h2 = Arc::new(Http2Session::new_buf_server(PolyStream::from(stream), 8 * 1024));
            h2.read_preface().await?;
            h2.send_settings(H2SETTINGS).await?;
            loop {
                if let Some(id) = h2.next().await? {
                    let http = DynHttpSocket::Http2(Http2Socket::new(id, h2.clone())?);
                    let hand = self.handler.clone();

                    tokio::spawn(async move {
                        match hand.entry(http).await {
                            Ok(()) => (),
                            Err(err) => eprintln!("{err}"),
                        }
                    });
                }
                if h2.goaway.load(std::sync::atomic::Ordering::Relaxed) {
                    break;
                }
            }
        }
        else {
            let mut http1 = Http1Socket::new(PolyStream::from(stream), 8 * 1024);
            let client = http1.read_until_head_complete().await?;
            
            if 
                self.allow_h2c && 
                let Some(up) = client.headers.get("upgrade") && 
                up[0].to_lowercase() == "h2c" 
            {
                let h2c = Arc::new(http1.h2c(Some(H2SETTINGS)).await?);
                h2c.read_preface().await?;
                h2c.send_settings(H2SETTINGS).await?;
                loop {
                    if let Some(id) = h2c.next().await? {
                        let http = DynHttpSocket::Http2(Http2Socket::new(id, h2c.clone())?);
                        let hand = self.handler.clone();

                        tokio::spawn(async move {
                            match hand.entry(http).await {
                                Ok(()) => (),
                                Err(err) => eprintln!("{err}"),
                            }
                        });
                    }
                    if h2c.goaway.load(std::sync::atomic::Ordering::Relaxed) {
                        break;
                    }
                }
            }
            else {
                http1.version_override = self.overide.clone();
                self.handler.clone().entry(http1.into()).await?;
            }
        }

        Ok(())
    }
}

pub struct Http2Server {
    listener: TcpListener,
    handler: Arc<dyn HttpHandler + Send + Sync + 'static>,
}
impl Http2Server {
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
    pub async fn handle(self: Arc<Self>, stream: TcpStream, addr: SocketAddr) -> Result<(), LibError> {
        dbg!(&addr);

        let h2 = Arc::new(Http2Session::new_buf_server(PolyStream::from(stream), 8 * 1024));
        h2.read_preface().await?;
        h2.send_settings(H2SETTINGS).await?;
        loop {
            if let Some(id) = h2.next().await? {
                let http = DynHttpSocket::Http2(Http2Socket::new(id, h2.clone())?);
                let hand = self.handler.clone();

                tokio::spawn(async move {
                    match hand.entry(http).await {
                        Ok(()) => (),
                        Err(err) => eprintln!("{err}"),
                    }
                });
            }
            if h2.goaway.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
        }

        Ok(())
    }
}


#[cfg(feature = "unix-sockets")]
pub struct UnixServer {
    listener: UnixListener,
    handler: Arc<dyn HttpHandler + Send + Sync + 'static>,
}
#[cfg(feature = "unix-sockets")]
impl UnixServer {
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
    pub async fn handle(self: Arc<Self>, stream: UnixStream, addr: tokio::net::unix::SocketAddr) -> Result<(), LibError> {
        dbg!(&addr);

        let mut http1 = Http1Socket::new(PolyStream::from(stream), 8 * 1024);
        http1.read_until_head_complete().await?;
        self.handler.clone().entry(http1.into()).await?;

        Ok(())
    }
}