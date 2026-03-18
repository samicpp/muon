use std::{net::SocketAddr, sync::Arc};

use http::{extra::PolyHttpSocket, ffihttp::{DynStream, PROVIDER, servers::TlsCertSelector}, http1::server::Http1Socket, http2::{core::Http2Settings, server::Http2Socket, session::Http2Session}, shared::{HttpMethod, HttpType, HttpVersion, LibError}};
use rustls::{pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject}, sign::CertifiedKey};
#[cfg(feature = "unix-sockets")]
use tokio::net::UnixListener;
use tokio::{io::{BufReader, ReadHalf, WriteHalf}, net::{TcpListener, TcpStream}, task::JoinHandle};
use tokio_rustls::TlsAcceptor;

use crate::{arguments::Cli, handlers::{HttpHandler, debug::DebugHandler, simple::SimpleHandler}, settings::Settings};


pub static H2SETTINGS: Http2Settings = Http2Settings::default_no_push();


pub async fn start_servers(args: Arc<Cli>, settings: Arc<Settings>) {
    let addresses = args.addresses.as_ref().map(|v| v.as_slice()).unwrap_or(&[]).iter().chain(settings.network.address.get().iter()).collect::<Vec<&String>>();

    let handler = settings.content.handler.as_deref().or(args.handler.as_deref()).unwrap_or("simple");

    let handler: Arc<dyn HttpHandler + Send + Sync + 'static> = 
    match handler {
        #[cfg(debug_assertions)]
        "debug" => Arc::new(DebugHandler),
        "simple" => Arc::new(SimpleHandler { _args: args.clone(), settings: settings.clone() }),

        _ => {
            eprintln!("no handler named {} available", handler);
            return
        }
    };

    // let mut servers = Vec::with_capacity(addresses.len());
    let mut jhs = Vec::with_capacity(addresses.len());

    let mut sni_builder = TlsCertSelector::new();
    if 
        let Some(cert_path) = &settings.network.default_cert && 
        let Some(key_path) = &settings.network.default_key &&
        let Ok(cert) = tokio::fs::read(cert_path).await &&
        let Ok(key) = tokio::fs::read(key_path).await &&
        let Ok(certs) = CertificateDer::pem_reader_iter(cert.as_slice()).map(|c| c.and_then(|c| Ok(c.into_owned()))).collect() &&
        let Ok(key) = PrivateKeyDer::from_pem_reader(key.as_slice()) &&
        let Ok(cert) = CertifiedKey::from_der(certs, key, &PROVIDER)
    {
        sni_builder.default = Some(Arc::new(cert));
    }

    for sni in &settings.network.sni {
        if 
            let Ok(cert) = tokio::fs::read(&sni.cert).await &&
            let Ok(key) = tokio::fs::read(&sni.key).await &&
            let Ok(certs) = CertificateDer::pem_reader_iter(cert.as_slice()).map(|c| c.and_then(|c| Ok(c.into_owned()))).collect() &&
            let Ok(key) = PrivateKeyDer::from_pem_reader(key.as_slice()) &&
            let Ok(cert) = CertifiedKey::from_der(certs, key, &PROVIDER)
        {
            sni_builder.add_cert(sni.domain.clone(), cert);
        }
    }

    let mut tls_config = sni_builder.to_server_conf();
    
    if let Some(alpn) = &settings.network.alpn {
        let converted = alpn.get().iter().map(|alpn| alpn.as_bytes().to_vec()).collect();
        tls_config.alpn_protocols = converted;
    }

    let tls_acceptor = Arc::new(TlsAcceptor::from(Arc::new(tls_config)));
    

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

            "https" => {
                if let Err(err) = start_tls(&mut jhs, loc, tls_acceptor.clone(), handler.clone(), true, false, None).await {
                    eprintln!("couldnt listen to {loc}");
                    eprintln!("{err}")
                }
            },
            "httpx" => {
                if let Err(err) = start_dyn_tls(&mut jhs, loc, tls_acceptor.clone(), handler.clone(), true, true, None).await {
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
    async fn accept(&self) -> std::io::Result<(DynStream, GenAddr)>;
}
impl Listener for TcpListener {
    async fn accept(&self) -> std::io::Result<(DynStream, GenAddr)> {
        let (stream, addr) = self.accept().await?;
        Ok((stream.into(), addr.into()))
    }
}
impl Listener for UnixListener {
    async fn accept(&self) -> std::io::Result<(DynStream, GenAddr)> {
        let (stream, addr) = self.accept().await?;
        Ok((stream.into(), addr.into()))
    }
}

pub struct TlsListener {
    listener: TcpListener,
    acceptor: Arc<TlsAcceptor>,
}
impl Listener for TlsListener {
    async fn accept(&self) -> std::io::Result<(DynStream, GenAddr)> {
        let (tcp, addr) = self.listener.accept().await?;
        let tls = self.acceptor.accept(tcp).await?;
        Ok((tls.into(), addr.into()))
    }
}


pub async fn start_tcp<A: tokio::net::ToSocketAddrs>(jhs: &mut Vec<JoinHandle<()>>, addr: A, handler: Arc<dyn HttpHandler + Send + Sync + 'static>, allow_h2c: bool, allow_prior_knowledge: bool, /*peek: bool,*/ assume: Option<HttpVersion>) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    
    jhs.push(tokio::spawn(serve(listener, handler, allow_h2c, allow_prior_knowledge, /*peek,*/ assume)));
    
    Ok(())
}
pub async fn start_tls<A: tokio::net::ToSocketAddrs>(jhs: &mut Vec<JoinHandle<()>>, addr: A, acceptor: Arc<TlsAcceptor>, handler: Arc<dyn HttpHandler + Send + Sync + 'static>, allow_h2c: bool, allow_prior_knowledge: bool, /*peek: bool,*/ assume: Option<HttpVersion>) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    let listener = TlsListener { listener, acceptor };
    
    jhs.push(tokio::spawn(serve(listener, handler, allow_h2c, allow_prior_knowledge, /*peek,*/ assume)));
    
    Ok(())
}
pub async fn start_dyn_tls<A: tokio::net::ToSocketAddrs>(jhs: &mut Vec<JoinHandle<()>>, addr: A, acceptor: Arc<TlsAcceptor>, handler: Arc<dyn HttpHandler + Send + Sync + 'static>, allow_h2c: bool, allow_prior_knowledge: bool, /*peek: bool,*/ assume: Option<HttpVersion>) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    
    jhs.push(tokio::spawn(async move {
        loop {
            let Ok((tcp, addr)) = listener.accept().await else { continue; };
            let handler = handler.clone();

            let assume = assume.clone();
            let acceptor = acceptor.clone();
            tokio::spawn(async move {
                match dyn_upgrade(tcp, acceptor).await {
                    Ok(stream) => match handle(handler, stream, addr.into(), allow_h2c, allow_prior_knowledge, /*peek,*/ assume).await {
                        Ok(()) => (),
                        Err(err) => eprintln!("{err}"),
                    },
                    Err(err) => {
                        eprintln!("{err}")
                    },
                }
            });
        }
    }));
    
    Ok(())
}
pub async fn dyn_upgrade(tcp: TcpStream, acceptor: Arc<TlsAcceptor>) -> Result<DynStream, std::io::Error> {
    let mut byte = [0];
    tcp.peek(&mut byte).await?;

    if byte[0] == 22 {
        let tls = acceptor.accept(tcp).await?;
        Ok(tls.into())
    }
    else {
        Ok(tcp.into())
    }

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

fn alpn_match(alpn: &[u8]) -> Option<HttpVersion> {
    #[cfg(debug_assertions)] println!("matching alpn {}", String::from_utf8_lossy(alpn));

    match alpn {
        b"h2" => Some(HttpVersion::Http2),
        b"http/1.1" => Some(HttpVersion::Http11),
        // b"http/1.0" => Some(HttpVersion::Http10), // unofficial
        // b"http/0.9" => Some(HttpVersion::Http09), // unofficial

        _ => None,
    }
}

pub async fn handle(
    handler: Arc<dyn HttpHandler + Send + Sync + 'static>, 
    
    mut stream: DynStream, 
    addr: GenAddr,

    allow_h2c: bool,
    allow_prior_knowledge: bool,

    // peek: bool,
    assume: Option<HttpVersion>,
) -> Result<(), LibError> {
    dbg!(&addr);
    
    let mut peek = [0; 24];

    match &mut stream {
        DynStream::Tcp(tcp) => tcp.peek(&mut peek).await?,
        _ => 0,
    };

    let assume =
    match &stream {
        DynStream::TlsDuplex(tls) => {
            let (_, info) = tls.get_ref();
            alpn_match(info.alpn_protocol().unwrap_or(&[]))
        },
        DynStream::TcpTls(tls) => {
            let (_, info) = tls.get_ref();
            alpn_match(info.alpn_protocol().unwrap_or(&[]))
        },
        DynStream::UnixTls(tls) => {
            let (_, info) = tls.get_ref();
            alpn_match(info.alpn_protocol().unwrap_or(&[]))
        },
        _ => assume,
    };

    if let Some(assumed) = &assume {
        match assumed.associated_type() {
            Some(HttpType::Http3) => unreachable!("http3 not yet implemented"),
            Some(HttpType::Http2) => {
                let http2 = Arc::new(Http2Session::new_buf_server(stream, 8 * 1024));
                http2.read_preface().await?; // TODO: send protocol violation if false
                http2.send_settings(H2SETTINGS).await?;
                h2_loop(handler, http2, addr).await?;
            },
            _ => {
                let http1 = Http1Socket::new(stream, 8 * 1024);
                if allow_h2c { possible_h2c(handler, http1, addr, assume).await?; }
                else {
                    handler.entry(http1.into(), addr).await?;
                }
            }
        }
    }
    else if allow_prior_knowledge && peek == http::http2::PREFACE {        
        let h2 = Arc::new(Http2Session::new_buf_server(DynStream::from(stream), 8 * 1024));
        h2.read_preface().await?;
        h2.send_settings(H2SETTINGS).await?;
        h2_loop(handler, h2, addr).await?;
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
            h2_loop(handler, http2, addr).await?;
        }

        else if allow_h2c { possible_h2c(handler, http1, addr, None).await?; }
        else { handler.entry(http1.into(), addr).await?; }
    }

    Ok(())
}

pub async fn h2_loop(handler: Arc<dyn HttpHandler + Send + Sync + 'static>, h2: Arc<Http2Session<BufReader<ReadHalf<DynStream>>, WriteHalf<DynStream>>>, addr: GenAddr) -> Result<(), LibError> {
    loop {
        match h2.next().await {
            Ok(Some(id)) => {
                let http = PolyHttpSocket::Http2(Http2Socket::new(id, h2.clone())?);
                let hand = handler.clone();
                let addr = addr.clone();

                tokio::spawn(async move {
                    match hand.entry(http, addr).await {
                        Ok(()) => (),
                        Err(err) => eprintln!("{err}"),
                    }
                });
            },
            Ok(None) => (),
            Err(err) => {
                eprintln!("{err}");
                h2.send_goaway(0, 1, b"protocol error").await?;
                break;
            }
        }
        if h2.goaway.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }
    }

    Ok(())
}
pub async fn possible_h2c(handler: Arc<dyn HttpHandler + Send + Sync + 'static>, mut http1: Http1Socket<ReadHalf<DynStream>, WriteHalf<DynStream>>, addr: GenAddr, verover: Option<HttpVersion>) -> Result<(), LibError> {
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
        let adr = addr.clone();

        tokio::spawn(async move {
            match hand.entry(http, adr).await {
                Ok(()) => (),
                Err(err) => eprintln!("{err}"),
            }
        });

        h2_loop(handler, h2c, addr).await?;
    }
    else {
        http1.version_override = verover;
        handler.entry(http1.into(), addr).await?;
    }

    Ok(())
}
