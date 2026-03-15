use std::{net::SocketAddr, sync::Arc};

use http::{extra::PolyHttpSocket, http1::server::Http1Socket, http2::{core::Http2Settings, server::Http2Socket, session::Http2Session}, shared::{HttpMethod, HttpType, HttpVersion, LibError}};
#[cfg(feature = "unix-sockets")]
use tokio::net::UnixListener;
use tokio::{io::{BufReader, ReadHalf, WriteHalf}, net::TcpListener, task::JoinHandle};

use crate::{arguments::Cli, handlers::{HttpHandler, debug::DebugHandler}, settings::Settings, stream::PolyStream};


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
                if let Err(err) = start_tcp(&mut jhs, loc, handler.clone(), true, true, None).await {
                    eprintln!("couldnt listen to {loc}");
                    eprintln!("{err}")
                }
            },
            "http1" => {
                if let Err(err) = start_tcp(&mut jhs, loc, handler.clone(), false, false, None).await {
                    eprintln!("couldnt listen to {loc}");
                    eprintln!("{err}")
                }
            },
            "http1.1" => {
                if let Err(err) = start_tcp(&mut jhs, loc, handler.clone(), false, false, Some(HttpVersion::Http11)).await {
                    eprintln!("couldnt listen to {loc}");
                    eprintln!("{err}")
                }
            },
            "http1.0" => {
                if let Err(err) = start_tcp(&mut jhs, loc, handler.clone(), false, false, Some(HttpVersion::Http10)).await {
                    eprintln!("couldnt listen to {loc}");
                    eprintln!("{err}")
                }
            },
            "http0.9" => {
                if let Err(err) = start_tcp(&mut jhs, loc, handler.clone(), false, false, Some(HttpVersion::Http09)).await {
                    eprintln!("couldnt listen to {loc}");
                    eprintln!("{err}")
                }
            },

            "http2" => {
                if let Err(err) = start_tcp(&mut jhs, loc, handler.clone(), false, false, Some(HttpVersion::Http2)).await {
                    eprintln!("couldnt listen to {loc}");
                    eprintln!("{err}")
                }
            },

            #[cfg(feature = "unix-sockets")]
            "unix" => {
                match UnixListener::bind(loc) {
                    Err(err) => eprintln!("{err}"),
                    Ok(listener) => {
                        // servers.push(Server::TcpH2(server.clone()));
                        let handler = handler.clone();
                        jhs.push(tokio::spawn(serve(listener, handler, true, true, None)));
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



#[allow(unused)]
#[derive(Debug, Clone)]
pub enum GenAddr {
    Norm(SocketAddr),
    #[cfg(feature = "unix-sockets")]
    Unix(tokio::net::unix::SocketAddr),
}
impl From<SocketAddr> for GenAddr {
    fn from(value: SocketAddr) -> Self {
        Self::Norm(value)
    }
}
#[cfg(feature = "unix-sockets")]
impl From<tokio::net::unix::SocketAddr> for GenAddr {
    fn from(value: tokio::net::unix::SocketAddr) -> Self {
        Self::Unix(value)
    }
}

pub trait Listener {
    async fn accept(&self) -> std::io::Result<(PolyStream, GenAddr)>;
}
impl Listener for TcpListener {
    async fn accept(&self) -> std::io::Result<(PolyStream, GenAddr)> {
        let (stream, addr) = self.accept().await?;
        Ok((stream.into(), addr.into()))
    }
}
impl Listener for UnixListener {
    async fn accept(&self) -> std::io::Result<(PolyStream, GenAddr)> {
        let (stream, addr) = self.accept().await?;
        Ok((stream.into(), addr.into()))
    }
}

pub async fn start_tcp<A: tokio::net::ToSocketAddrs>(jhs: &mut Vec<JoinHandle<()>>, addr: A, handler: Arc<dyn HttpHandler + Send + Sync + 'static>, allow_h2c: bool, allow_prior_knowledge: bool, /*peek: bool,*/ assume: Option<HttpVersion>) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    
    jhs.push(tokio::spawn(serve(listener, handler, allow_h2c, allow_prior_knowledge, /*peek,*/ assume)));
    
    Ok(())
}
pub async fn serve<L: Listener>(listener: L, handler: Arc<dyn HttpHandler + Send + Sync + 'static>, allow_h2c: bool, allow_prior_knowledge: bool, /*peek: bool,*/ assume: Option<HttpVersion>) {
    loop {
        let Ok((stream, addr)) = listener.accept().await else { continue; };
        let handler = handler.clone();

        let assume = assume.clone();
        tokio::spawn(async move {
            match handle(handler, stream.into(), addr.into(), allow_h2c, allow_prior_knowledge, /*peek,*/ assume).await {
                Ok(()) => (),
                Err(err) => eprintln!("{err}"),
            }
        });
    }
}

pub async fn handle(
    handler: Arc<dyn HttpHandler + Send + Sync + 'static>, 
    
    mut stream: PolyStream, 
    addr: GenAddr,

    allow_h2c: bool,
    allow_prior_knowledge: bool,

    // peek: bool,
    assume: Option<HttpVersion>,
) -> Result<(), LibError> {
    dbg!(&addr);
    
    let mut peek = [0; 24];

    match &mut stream {
        PolyStream::Tcp(tcp) => tcp.peek(&mut peek).await?,
        _ => unreachable!(),
    };

    if let Some(assumed) = &assume {
        match assumed.associated_type() {
            Some(HttpType::Http3) => unreachable!("http3 not yet implemented"),
            Some(HttpType::Http2) => {
                let http2 = Arc::new(Http2Session::new_buf_server(stream, 8 * 1024));
                http2.read_preface().await?; // TODO: send protocol violation if false
                http2.send_settings(H2SETTINGS).await?;
                h2_loop(handler, http2).await?;
            },
            _ => {
                let http1 = Http1Socket::new(stream, 8 * 1024);
                if allow_h2c { possible_h2c(handler, http1, assume).await?; }
                else {
                    handler.entry(http1.into()).await?;
                }
            }
        }
    }
    else if allow_prior_knowledge && peek == http::http2::PREFACE {        
        let h2 = Arc::new(Http2Session::new_buf_server(PolyStream::from(stream), 8 * 1024));
        h2.read_preface().await?;
        h2.send_settings(H2SETTINGS).await?;
        h2_loop(handler, h2).await?;
    }
    else {
        let mut http1 = Http1Socket::new(stream, 8 * 1024);
        let client = http1.read_until_head_complete().await?;

        if allow_prior_knowledge && 
            client.method == HttpMethod::Unknown(Some("PRI".to_owned())) && 
            client.path == "*" && 
            client.version == HttpVersion::Unknown(Some("HTTP/2.0".to_owned())) && 
            client.headers.len() == 0 
        {
            let http2 = Arc::new(http1.http2_prior_knowledge().await?);
            http2.send_settings(H2SETTINGS).await?;
            h2_loop(handler, http2).await?;
        }

        else if allow_h2c { possible_h2c(handler, http1, None).await?; }
        else { handler.entry(http1.into()).await?; }
    }

    Ok(())
}

pub async fn h2_loop(handler: Arc<dyn HttpHandler + Send + Sync + 'static>, h2: Arc<Http2Session<BufReader<ReadHalf<PolyStream>>, WriteHalf<PolyStream>>>) -> Result<(), LibError> {
    loop {
        if let Some(id) = h2.next().await? { // TODO: properly handle the error so a protocol violation can be sent
            let http = PolyHttpSocket::Http2(Http2Socket::new(id, h2.clone())?);
            let hand = handler.clone();

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
pub async fn possible_h2c(handler: Arc<dyn HttpHandler + Send + Sync + 'static>, mut http1: Http1Socket<ReadHalf<PolyStream>, WriteHalf<PolyStream>>, verover: Option<HttpVersion>) -> Result<(), LibError> {
    let client = http1.read_until_head_complete().await?;
    
    if 
        let Some(up) = client.headers.get("upgrade") && 
        up[0].to_lowercase() == "h2c" 
    {
        let h2c = Arc::new(http1.h2c(Some(H2SETTINGS)).await?);
        h2c.read_preface().await?;
        h2c.send_settings(H2SETTINGS).await?;

        let http = PolyHttpSocket::Http2(Http2Socket::new(1, h2c.clone()).unwrap());
        let hand = handler.clone();

        tokio::spawn(async move {
            match hand.entry(http).await {
                Ok(()) => (),
                Err(err) => eprintln!("{err}"),
            }
        });

        h2_loop(handler, h2c).await?;
    }
    else {
        http1.version_override = verover;
        handler.entry(http1.into()).await?;
    }

    Ok(())
}
