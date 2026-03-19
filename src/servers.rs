use std::{net::SocketAddr, sync::Arc, time::{Duration, Instant}};

use http::{extra::PolyHttpSocket, ffihttp::{DynStream, PROVIDER, servers::TlsCertSelector}, http1::server::Http1Socket, http2::{core::Http2Settings, server::Http2Socket, session::Http2Session}, shared::{HttpMethod, HttpType, HttpVersion, LibError}};
use rustls::{pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject}, sign::CertifiedKey};
#[cfg(feature = "unix-sockets")]
use tokio::net::UnixListener;
use tokio::{io::{BufReader, ReadHalf, WriteHalf}, net::{TcpListener, TcpSocket, TcpStream}};
use tokio_rustls::TlsAcceptor;
use owo_colors::OwoColorize;
#[cfg(debug_assertions)]
use crate::handlers::debug::DebugHandler;
use crate::{arguments::Cli, elog_with_level, handlers::{HttpHandler, samicpp::SamicppHandler, simple::SimpleHandler}, logger::{check_loglevel, loglevels}, settings::Settings};


pub static H2SETTINGS: Http2Settings = Http2Settings::default_no_push();


pub async fn start_servers(args: Arc<Cli>, settings: Arc<Settings>) {
    let addresses = args.addresses.as_ref().map(|v| v.as_slice()).unwrap_or(&[]).iter().chain(settings.network.address.get().iter()).collect::<Vec<&String>>();

    let handler = settings.content.handler.as_deref().or(args.handler.as_deref()).unwrap_or("simple");

    let handler: Arc<dyn HttpHandler + Send + Sync + 'static> = 
    match handler {
        #[cfg(debug_assertions)]
        "debug" => Arc::new(DebugHandler),
        "simple" => Arc::new(SimpleHandler { _args: args.clone(), settings: settings.clone() }),
        "samicpp" => Arc::new(SamicppHandler::new(args.clone(), settings.clone())),

        _ => {
            elog_with_level!(loglevels::INIT_ERROR, "no handler named {} available", handler);
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
    
    if let Some(alpn) = args.alpn.as_deref() {
        let converted = alpn.split(',').map(|alpn| alpn.as_bytes().to_vec()).collect();
        tls_config.alpn_protocols = converted;
    }
    else if let Some(alpn) = &settings.network.alpn {
        let converted = alpn.get().iter().map(|alpn| alpn.as_bytes().to_vec()).collect();
        tls_config.alpn_protocols = converted;
    }

    let tls_config = Arc::new(tls_config);
    let tls_acceptor = Arc::new(TlsAcceptor::from(tls_config.clone()));
    
    let backlog = settings.network.backlog.unwrap_or(1024);

    for addr in addresses {
        let mut pl = addr.splitn(2, "://");

        let Some(prot) = pl.next() else { continue; };
        let Some(loc) = pl.next() else {
            elog_with_level!(loglevels::INIT_ERROR, "invalid address: \"{addr}\"");
            continue;
        };

        match prot {
            "tcp" | "http" => {
                match create_socket(loc, backlog) {
                    Err(err) => elog_with_level!(loglevels::INIT_ERROR, "couldnt listen to {loc} {}", err.red()),
                    Ok(listener) => jhs.push(tokio::spawn(start_tcp(listener, handler.clone(), true, true, None))),
                }
            },
            "http1" => {
                match create_socket(loc, backlog) {
                    Err(err) => elog_with_level!(loglevels::INIT_ERROR, "couldnt listen to {loc} {}", err.red()),
                    Ok(listener) => jhs.push(tokio::spawn(start_tcp(listener, handler.clone(), false, false, None))),
                }
            },
            "http1.1" => {
                match create_socket(loc, backlog) {
                    Err(err) => elog_with_level!(loglevels::INIT_ERROR, "couldnt listen to {loc} {}", err.red()),
                    Ok(listener) => jhs.push(tokio::spawn(start_tcp(listener, handler.clone(), false, false, Some(HttpVersion::Http11)))),
                }
            },
            "http1.0" => {
                match create_socket(loc, backlog) {
                    Err(err) => elog_with_level!(loglevels::INIT_ERROR, "couldnt listen to {loc} {}", err.red()),
                    Ok(listener) => jhs.push(tokio::spawn(start_tcp(listener, handler.clone(), false, false, Some(HttpVersion::Http10)))),
                }
            },
            "http0.9" => {
                match create_socket(loc, backlog) {
                    Err(err) => elog_with_level!(loglevels::INIT_ERROR, "couldnt listen to {loc} {}", err.red()),
                    Ok(listener) => jhs.push(tokio::spawn(start_tcp(listener, handler.clone(), false, false, Some(HttpVersion::Http09)))),
                }
            },

            "http2" => {
                match create_socket(loc, backlog) {
                    Err(err) => elog_with_level!(loglevels::INIT_ERROR, "couldnt listen to {loc} {}", err.red()),
                    Ok(listener) => jhs.push(tokio::spawn(start_tcp(listener, handler.clone(), false, false, Some(HttpVersion::Http2)))),
                }
            },

            "https" => {
                match create_socket(loc, backlog) {
                    Err(err) => elog_with_level!(loglevels::INIT_ERROR, "couldnt listen to {loc} {}", err.red()),
                    Ok(listener) => jhs.push(tokio::spawn(start_tls(listener, tls_acceptor.clone(), handler.clone(), true, false, None))),
                }
            },
            "httpx" => {
                match create_socket(loc, backlog) {
                    Err(err) => elog_with_level!(loglevels::INIT_ERROR, "couldnt listen to {loc} {}", err.red()),
                    Ok(listener) => jhs.push(tokio::spawn(start_dyn_tls(listener, tls_acceptor.clone(), handler.clone(), true, true, None))),
                }
            },

            #[cfg(feature = "unix-sockets")]
            "unix" => {
                match UnixListener::bind(loc) {
                    Err(err) => elog_with_level!(loglevels::INIT_ERROR, "couldnt listen to {loc} {}", err.red()),
                    Ok(listener) => {
                        // servers.push(Server::TcpH2(server.clone()));
                        let handler = handler.clone();
                        jhs.push(tokio::spawn(serve(listener, handler, true, true, None)));
                    }
                }
            }

            _ => elog_with_level!(loglevels::INIT_ERROR, "invalid protocol \"{prot}\""),
        }
    }

    for binding in &settings.network.binding {
        let mut pl = binding.address.splitn(2, "://");

        let Some(prot) = pl.next() else { continue; };
        let Some(loc) = pl.next() else {
            elog_with_level!(loglevels::INIT_ERROR, "invalid address: \"{}\"", binding.address);
            continue;
        };

        let Ok(Some(address)) = std::net::ToSocketAddrs::to_socket_addrs(loc).map(|mut i| i.next()) else { continue };
        let socket = if address.is_ipv4() { TcpSocket::new_v4() } else { TcpSocket::new_v6() };
        let socket = match socket {
            Ok(socket) => socket,
            Err(err) => {
                elog_with_level!(loglevels::INIT_ERROR, "couldnt create socket {}", err.red());
                continue;
            }
        };

        if let Some(opt) = binding.reuse_addr { let _ = socket.set_reuseaddr(opt); }
        if let Some(opt) = binding.reuse_port { let _ = socket.set_reuseport(opt); }
        if let Some(opt) = binding.nodelay { let _ = socket.set_nodelay(opt); }
        if let Some(opt) = binding.recv_bufsize { let _ = socket.set_recv_buffer_size(opt); }
        if let Some(opt) = binding.send_bufsize { let _ = socket.set_send_buffer_size(opt); }

        if let Err(err) = socket.bind(address) {
            elog_with_level!(loglevels::INIT_ERROR, "couldnt bind {}", err.red());
            continue;
        }
        let listener = match socket.listen(binding.backlog.unwrap_or(backlog)) {
            Ok(socket) => socket,
            Err(err) => {
                elog_with_level!(loglevels::INIT_ERROR, "couldnt listen {}", err.red());
                continue;
            }
        };

        let allow_h2c;
        let allow_prior_knowledge;
        let assume;
        let dyn_tls;

        match prot {
            "tcp" | "http" => {
                allow_h2c = true;
                allow_prior_knowledge = true;
                assume = None;
                dyn_tls = None;
            },
            "http1" => {
                allow_h2c = false;
                allow_prior_knowledge = false;
                assume = None;
                dyn_tls = None;
            },
            "http1.1" => {
                allow_h2c = false;
                allow_prior_knowledge = false;
                assume = Some(HttpVersion::Http11);
                dyn_tls = None;
            },
            "http1.0" => {
                allow_h2c = false;
                allow_prior_knowledge = false;
                assume = Some(HttpVersion::Http10);
                dyn_tls = None;
            },
            "http0.9" => {
                allow_h2c = false;
                allow_prior_knowledge = false;
                assume = Some(HttpVersion::Http09);
                dyn_tls = None;
            },

            "http2" => {
                allow_h2c = false;
                allow_prior_knowledge = false;
                assume = Some(HttpVersion::Http2);
                dyn_tls = None;
            },

            "https" => {
                allow_h2c = false;
                allow_prior_knowledge = false;
                assume = None;
                dyn_tls = Some(false);
            },
            "httpx" => {
                allow_h2c = true;
                allow_prior_knowledge = true;
                assume = None;
                dyn_tls = Some(true);
            },

            _ => {
                elog_with_level!(loglevels::INIT_ERROR, "invalid protocol \"{prot}\"");
                continue;
            }
        }

        let acceptor = tls_acceptor.clone();
        let handler = handler.clone();
        let assume = assume.clone();
        match dyn_tls {
            Some(true) => {
                jhs.push(tokio::spawn(start_dyn_tls(listener, acceptor, handler, allow_h2c, allow_prior_knowledge, assume)));
            },
            Some(false) => {
                jhs.push(tokio::spawn(start_tls(listener, acceptor, handler, allow_h2c, allow_prior_knowledge, assume)));
            },
            None => jhs.push(tokio::spawn(start_tcp(listener, handler, allow_h2c, allow_prior_knowledge, assume))),
        }
    }

    for jh in jhs {
        let _ = jh.await;
    }
}


fn timestamp(elapsed: Duration) -> String {
    let nanos = elapsed.as_nanos();
    let micros = nanos / 1_000;
    let milis = micros / 1_000;
    let sec = milis / 1_000;
    let min = sec / 60;
    let hours = min / 60;
    let days = hours / 24;

    let mut stamp = String::new();
    if days > 0 {
        stamp.push_str(&format!("{days}d"));
    }
    if hours > 0 {
        stamp.push_str(&format!(" {}d", hours % 24));
    }
    if min > 0 {
        stamp.push_str(&format!(" {}m", min % 60));
    }
    if sec > 0 {
        stamp.push_str(&format!(" {}s", sec % 60));
    }
    if milis > 0 {
        stamp.push_str(&format!(" {}ms", milis % 1000));
    }
    stamp.push_str(&format!(" {}μs {}ns", micros % 1_000, nanos % 1_000));

    stamp
}


#[allow(unused)]
#[derive(Debug, Clone)]
pub enum GenAddr {
    Net(SocketAddr),
    #[cfg(feature = "unix-sockets")]
    Unix(tokio::net::unix::SocketAddr),
}
impl From<SocketAddr> for GenAddr {
    fn from(value: SocketAddr) -> Self {
        Self::Net(value)
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

// pub struct TlsListener {
//     listener: TcpListener,
//     acceptor: Arc<TlsAcceptor>,
// }
// impl Listener for TlsListener {
//     async fn accept(&self) -> std::io::Result<(DynStream, GenAddr)> {
//         let (tcp, addr) = self.listener.accept().await?;
//         let tls = self.acceptor.accept(tcp).await?;
//         Ok((tls.into(), addr.into()))
//     }
// }

pub fn create_socket<A: std::net::ToSocketAddrs>(addr: A, backlog: u32) -> std::io::Result<TcpListener> {
    let Some(address) = addr.to_socket_addrs()?.next() else { 
        return Err(std::io::Error::new(std::io::ErrorKind::AddrNotAvailable, "invalid address")) 
    };

    let socket = 
    if address.is_ipv4() { 
        TcpSocket::new_v4() 
    } 
    else { 
        TcpSocket::new_v6() 
    }?;
    socket.bind(address)?;
    socket.listen(backlog)
}
pub async fn start_tcp(
    // jhs: &mut Vec<JoinHandle<()>>, 
    listener: TcpListener, 
    handler: Arc<dyn HttpHandler + Send + Sync + 'static>, 
    allow_h2c: bool, 
    allow_prior_knowledge: bool, 
    /*peek: bool,*/ 
    assume: Option<HttpVersion>,
) -> () {
    loop {
        let Ok((stream, addr)) = listener.accept().await else { continue; };
        let handler = handler.clone();

        let assume = assume.clone();
        tokio::spawn(async move {
            let now = Instant::now();
            match handle(handler, stream.into(), addr.into(), allow_h2c, allow_prior_knowledge, /*peek,*/ assume).await {
                Ok(()) => (),
                Err(err) => elog_with_level!(loglevels::HANDLER_ERROR, "{err}"),
            }
            if check_loglevel(loglevels::RESPONSE_TIME) {
                let stamp = timestamp(now.elapsed());
                println!("response took {}", &stamp);
            }
        });
    }
}
pub async fn start_tls(
    // jhs: &mut Vec<JoinHandle<()>>, 
    listener: TcpListener, 
    acceptor: Arc<TlsAcceptor>, 
    handler: Arc<dyn HttpHandler + Send + Sync + 'static>, 
    allow_h2c: bool, 
    allow_prior_knowledge: bool, 
    /*peek: bool,*/ 
    assume: Option<HttpVersion>,
) -> () {
    loop {
        let Ok((stream, addr)) = listener.accept().await else { continue; };
        let handler = handler.clone();

        let assume = assume.clone();
        let acceptor = acceptor.clone();
        tokio::spawn(async move {
            let now = Instant::now();
            match acceptor.accept(stream).await {
                Ok(tls) => match handle(handler, tls.into(), addr.into(), allow_h2c, allow_prior_knowledge, /*peek,*/ assume).await {
                    Ok(()) => (),
                    Err(err) => elog_with_level!(loglevels::HANDLER_ERROR, "{err}"),
                },
                Err(err) => {
                    elog_with_level!(loglevels::TLS_UPGRADE_ERROR, "{err}")
                }
            }
            if check_loglevel(loglevels::RESPONSE_TIME) {
                let stamp = timestamp(now.elapsed());
                println!("response took {}", &stamp);
            }
        });
    }
}
pub async fn start_dyn_tls(
    // jhs: &mut Vec<JoinHandle<()>>, 
    listener: TcpListener, 
    acceptor: Arc<TlsAcceptor>, 
    handler: Arc<dyn HttpHandler + Send + Sync + 'static>, 
    allow_h2c: bool, 
    allow_prior_knowledge: bool, 
    /*peek: bool,*/ 
    assume: Option<HttpVersion>,
) -> () {
    loop {
        let Ok((tcp, addr)) = listener.accept().await else { continue; };
        let handler = handler.clone();

        let assume = assume.clone();
        let acceptor = acceptor.clone();
        tokio::spawn(async move {
            let now = Instant::now();
            match dyn_upgrade(tcp, acceptor).await {
                Ok(stream) => match handle(handler, stream, addr.into(), allow_h2c, allow_prior_knowledge, /*peek,*/ assume).await {
                    Ok(()) => (),
                    Err(err) => elog_with_level!(loglevels::HANDLER_ERROR, "{err}"),
                },
                Err(err) => {
                    elog_with_level!(loglevels::TLS_UPGRADE_ERROR, "{err}")
                },
            }
            if check_loglevel(loglevels::RESPONSE_TIME) {
                let stamp = timestamp(now.elapsed());
                println!("response took {}", &stamp);
            }
        });
    }
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
            let now = Instant::now();
            match handle(handler, stream.into(), addr.into(), allow_h2c, allow_prior_knowledge, /*peek,*/ assume).await {
                Ok(()) => (),
                Err(err) => elog_with_level!(loglevels::HANDLER_ERROR, "{err}"),
            }
            if check_loglevel(loglevels::RESPONSE_TIME) {
                let stamp = timestamp(now.elapsed());
                println!("response took {}", &stamp);
            }
        });
    }
}

fn alpn_match(alpn: &[u8]) -> Option<HttpVersion> {
    #[cfg(debug_assertions)] println!("matching alpn {}", String::from_utf8_lossy(alpn));

    match alpn {
        b"h2" => Some(HttpVersion::Http2),
        b"http/1.1" => Some(HttpVersion::Http11),
        b"http/1.0" => Some(HttpVersion::Http10), // unofficial
        b"http/0.9" => Some(HttpVersion::Http09), // unofficial

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
    #[cfg(debug_assertions)] dbg!(&addr);
    
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
                    match handler.entry(http1.into(), addr).await {
                        Ok(()) => {},
                        Err(err) => elog_with_level!(loglevels::CONTENT_HANDLER_ERROR, "{err}"),
                    };
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
        else {
            http1.set_header("Connection", "close");
            match handler.entry(http1.into(), addr).await {
                Ok(()) => {},
                Err(err) => elog_with_level!(loglevels::CONTENT_HANDLER_ERROR, "{err}"),
            }
        }
    }

    Ok(())
}

pub async fn h2_loop(handler: Arc<dyn HttpHandler + Send + Sync + 'static>, h2: Arc<Http2Session<BufReader<ReadHalf<DynStream>>, WriteHalf<DynStream>>>, addr: GenAddr) -> Result<(), LibError> {
    loop {
        let frame = h2.read_frame().await?;
        if check_loglevel(loglevels::HTTP2_FRAME_DUMP) {
            println!("\x1b[36m{:?}\x1b[0m ({}) {:?}", frame.ftype, frame.source.len(), &frame.source[..29.min(frame.source.len())]);
        }
        match h2.handle(frame).await {
            Ok(Some(id)) => {
                let http = PolyHttpSocket::Http2(Http2Socket::new(id, h2.clone())?);
                let hand = handler.clone();
                let addr = addr.clone();

                tokio::spawn(async move {
                    match hand.entry(http, addr).await {
                        Ok(()) => (),
                        Err(err) => elog_with_level!(loglevels::CONTENT_HANDLER_ERROR, "{err}"),
                    }
                });
            },
            Ok(None) => (),
            Err(err @ (LibError::InvalidFrame | LibError::InvalidStream | LibError::ProtocolError | LibError::Huffman(_))) => {
                elog_with_level!(loglevels::HTTP2_ERROR, "{err}");
                h2.send_goaway(0, 1, b"protocol error").await?;
                break;
            },
            Err(err) => {
                elog_with_level!(loglevels::HTTP2_ERROR, "{err}");
                break;
            },
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
                Err(err) => elog_with_level!(loglevels::CONTENT_HANDLER_ERROR, "{err}"),
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
