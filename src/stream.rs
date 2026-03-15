use std::{ffi::c_void, pin::Pin, sync::{Arc, LazyLock}};

use http::shared::{LibError, Stream};
use httprs_core::ffi::{futures::FfiFuture, own::spawn_task};
use rustls::crypto::CryptoProvider;
#[cfg(feature = "unix-sockets")]
use tokio::net::UnixStream;
use tokio::{io::{AsyncRead, AsyncWrite, DuplexStream}, net::TcpStream};
use tokio_rustls::TlsStream;


#[derive(Debug)]
pub enum DynStream {
    Duplex(DuplexStream),
    TlsDuplex(TlsStream<DuplexStream>),
    Tcp(TcpStream),
    TcpTls(TlsStream<TcpStream>),
    #[cfg(feature = "unix-sockets")]
    Unix(UnixStream),
    #[cfg(feature = "unix-sockets")]
    UnixTls(TlsStream<UnixStream>),
}
impl DynStream{
    pub fn to_stream(self) -> Box<dyn Stream>{
        match self{
            Self::Tcp(tcp) => Box::new(tcp),
            Self::TcpTls(tls) => Box::new(tls),
            Self::Duplex(dup) => Box::new(dup),
            Self::TlsDuplex(dup) => Box::new(dup),
            #[cfg(feature = "unix-sockets")]
            Self::Unix(uni) => Box::new(uni),
            #[cfg(feature = "unix-sockets")]
            Self::UnixTls(uni) => Box::new(uni),
        }
    }

    pub fn is_duplex(&self) -> bool {
        if let Self::Duplex(_) = self { true }
        else { false }
    }
    pub fn is_tls_duplex(&self) -> bool {
        if let Self::TlsDuplex(_) = self { true }
        else { false }
    }
    pub fn is_tcp(&self) -> bool {
        if let Self::Tcp(_) = self { true }
        else { false }
    }
    pub fn is_tcp_tls(&self) -> bool {
        if let Self::TcpTls(_) = self { true }
        else { false }
    }
    #[cfg(feature = "unix-sockets")]
    pub fn is_unix(&self) -> bool {
        if let Self::Unix(_) = self { true }
        else { false }
    }
    #[cfg(feature = "unix-sockets")]
    pub fn is_unix_tls(&self) -> bool {
        if let Self::UnixTls(_) = self { true }
        else { false }
    }
}
impl From<TcpStream> for DynStream{
    fn from(value: TcpStream) -> Self {
        Self::Tcp(value)
    }
}
impl From<TlsStream<TcpStream>> for DynStream{
    fn from(value: TlsStream<TcpStream>) -> Self {
        Self::TcpTls(value)
    }
}
impl From<tokio_rustls::client::TlsStream<TcpStream>> for DynStream{
    fn from(value: tokio_rustls::client::TlsStream<TcpStream>) -> Self {
        Self::TcpTls(TlsStream::Client(value))
    }
}
impl From<tokio_rustls::server::TlsStream<TcpStream>> for DynStream{
    fn from(value: tokio_rustls::server::TlsStream<TcpStream>) -> Self {
        Self::TcpTls(TlsStream::Server(value))
    }
}
impl From<DuplexStream> for DynStream{
    fn from(value: DuplexStream) -> Self {
        Self::Duplex(value)
    }
}
impl From<TlsStream<DuplexStream>> for DynStream{
    fn from(value: TlsStream<DuplexStream>) -> Self {
        Self::TlsDuplex(value)
    }
}
impl From<tokio_rustls::client::TlsStream<DuplexStream>> for DynStream{
    fn from(value: tokio_rustls::client::TlsStream<DuplexStream>) -> Self {
        Self::TlsDuplex(TlsStream::Client(value))
    }
}
impl From<tokio_rustls::server::TlsStream<DuplexStream>> for DynStream{
    fn from(value: tokio_rustls::server::TlsStream<DuplexStream>) -> Self {
        Self::TlsDuplex(TlsStream::Server(value))
    }
}
impl AsyncRead for DynStream {
    fn poll_read(
            self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
            buf: &mut tokio::io::ReadBuf<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
        unsafe {
            match self.get_unchecked_mut() {
                Self::Tcp(tcp) => Pin::new_unchecked(tcp).poll_read(cx, buf),
                Self::TcpTls(tls) => Pin::new_unchecked(tls).poll_read(cx, buf),
                Self::Duplex(dup) => Pin::new_unchecked(dup).poll_read(cx, buf),
                Self::TlsDuplex(dup) => Pin::new_unchecked(dup).poll_read(cx, buf),
                #[cfg(feature = "unix-sockets")]
                Self::Unix(uni) => Pin::new_unchecked(uni).poll_read(cx, buf),
                #[cfg(feature = "unix-sockets")]
                Self::UnixTls(uni) => Pin::new_unchecked(uni).poll_read(cx, buf),
            }
        }
    }
}
impl AsyncWrite for DynStream {
    fn poll_flush(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<std::io::Result<()>> {
        unsafe {
            match self.get_unchecked_mut() {
                Self::Tcp(tcp) => Pin::new_unchecked(tcp).poll_flush(cx),
                Self::TcpTls(tls) => Pin::new_unchecked(tls).poll_flush(cx),
                Self::Duplex(dup) => Pin::new_unchecked(dup).poll_flush(cx),
                Self::TlsDuplex(dup) => Pin::new_unchecked(dup).poll_flush(cx),
                #[cfg(feature = "unix-sockets")]
                Self::Unix(uni) => Pin::new_unchecked(uni).poll_flush(cx),
                #[cfg(feature = "unix-sockets")]
                Self::UnixTls(uni) => Pin::new_unchecked(uni).poll_flush(cx),
            }
        }
    }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<std::io::Result<()>> {
        unsafe {
            match self.get_unchecked_mut() {
                Self::Tcp(tcp) => Pin::new_unchecked(tcp).poll_shutdown(cx),
                Self::TcpTls(tls) => Pin::new_unchecked(tls).poll_shutdown(cx),
                Self::Duplex(dup) => Pin::new_unchecked(dup).poll_shutdown(cx),
                Self::TlsDuplex(dup) => Pin::new_unchecked(dup).poll_shutdown(cx),
                #[cfg(feature = "unix-sockets")]
                Self::Unix(uni) => Pin::new_unchecked(uni).poll_shutdown(cx),
                #[cfg(feature = "unix-sockets")]
                Self::UnixTls(uni) => Pin::new_unchecked(uni).poll_shutdown(cx),
            }
        }
    }
    fn poll_write(
            self: Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
            buf: &[u8],
        ) -> std::task::Poll<std::io::Result<usize>> {
        unsafe {
            match self.get_unchecked_mut() {
                Self::Tcp(tcp) => Pin::new_unchecked(tcp).poll_write(cx, buf),
                Self::TcpTls(tls) => Pin::new_unchecked(tls).poll_write(cx, buf),
                Self::Duplex(dup) => Pin::new_unchecked(dup).poll_write(cx, buf),
                Self::TlsDuplex(dup) => Pin::new_unchecked(dup).poll_write(cx, buf),
                #[cfg(feature = "unix-sockets")]
                Self::Unix(uni) => Pin::new_unchecked(uni).poll_write(cx, buf),
                #[cfg(feature = "unix-sockets")]
                Self::UnixTls(uni) => Pin::new_unchecked(uni).poll_write(cx, buf),
            }
        }
    }
    fn poll_write_vectored(
            self: Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
            bufs: &[std::io::IoSlice<'_>],
        ) -> std::task::Poll<std::io::Result<usize>> {
        unsafe {
            match self.get_unchecked_mut() {
                Self::Tcp(tcp) => Pin::new_unchecked(tcp).poll_write_vectored(cx, bufs),
                Self::TcpTls(tls) => Pin::new_unchecked(tls).poll_write_vectored(cx, bufs),
                Self::Duplex(dup) => Pin::new_unchecked(dup).poll_write_vectored(cx, bufs),
                Self::TlsDuplex(dup) => Pin::new_unchecked(dup).poll_write_vectored(cx, bufs),
                #[cfg(feature = "unix-sockets")]
                Self::Unix(uni) => Pin::new_unchecked(uni).poll_write_vectored(cx, bufs),
                #[cfg(feature = "unix-sockets")]
                Self::UnixTls(uni) => Pin::new_unchecked(uni).poll_write_vectored(cx, bufs),
            }
        }
    }
    fn is_write_vectored(&self) -> bool {
        match self {
            Self::Tcp(tcp) => tcp.is_write_vectored(),
            Self::TcpTls(tls) => tls.is_write_vectored(),
            Self::Duplex(dup) => dup.is_write_vectored(),
            Self::TlsDuplex(dup) => dup.is_write_vectored(),
            #[cfg(feature = "unix-sockets")]
            Self::Unix(uni) => uni.is_write_vectored(),
            #[cfg(feature = "unix-sockets")]
            Self::UnixTls(uni) => uni.is_write_vectored(),
        }
    }
}