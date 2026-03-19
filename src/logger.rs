// use std::fmt::Display;

use std::{sync::atomic::AtomicU64, u64};

use http::shared::HttpClient;


// pub enum Color {
//     None,

//     Reset, // \e[0m

//     C8Bit(u8),
//     C24Bit(u8, u8, u8),
// }
// pub trait Colorize {
//     fn reset(&self) -> String;

//     fn color_fg_8(&self, color: u8) -> String;
//     fn color_fg_24(&self, r: u8, g: u8, b: u8) -> String;

//     fn color_bg_8(&self, color: u8) -> String;
//     fn color_bg_24(&self, r: u8, g: u8, b: u8) -> String;
// }
// impl<D: Display> Colorize for D {
//     fn reset(&self) -> String { format!("{self}\x1b[0m") }
//     fn color_fg_8(&self, color: u8) -> String { format!("\x1b[38;5;{color}m{self}") }
//     fn color_fg_24(&self, r: u8, g: u8, b: u8) -> String { format!("\x1b[38;2;{r};{g};{b}m{self}") }
//     fn color_bg_8(&self, color: u8) -> String { format!("\x1b[48;5;{color}m{self}") }
//     fn color_bg_24(&self, r: u8, g: u8, b: u8) -> String { format!("\x1b[48;2;{r};{g};{b}m{self}") }
// }

pub static LOGLEVEL: AtomicU64 = AtomicU64::new(u64::MAX);



pub fn log_client_simple(client: &HttpClient) -> String {
    use http::shared::HttpMethod::*;
    use http::shared::HttpVersion::*;

    format!(
        "{} {} {}",
        match &client.method {
            http::shared::HttpMethod::Unknown(Some(m)) => m,
            http::shared::HttpMethod::Unknown(None) => "UNKOWN",

            Get => "\x1b[32mGET\x1b[0m",
            Head => "\x1b[32mHEAD\x1b[0m",
            Post => "\x1b[33mPOST\x1b[0m",
            Put => "\x1b[33mPUT\x1b[0m",
            Delete => "\x1b[31mDELETE\x1b[0m",
            Connect => "\x1b[36mCONNECT\x1b[0m",
            Options => "\x1b[35mOPTIONS\x1b[0m",
            Trace => "\x1b[90mTRACE\x1b[0m",
        },
        &client.path,
        match &client.version {
            http::shared::HttpVersion::Unknown(Some(v)) => v,
            http::shared::HttpVersion::Unknown(None) => "UNKNOWN/0.0",
            Debug => "\x1b[90mDEBUG/0.0\x1b[0m",

            Http09 => "\x1b[31mHTTP/0.9\x1b[0m",
            Http10 => "\x1b[33mHTTP/1.0\x1b[0m",
            Http11 => "\x1b[33mHTTP/1.1\x1b[0m",
            Http2 => "\x1b[32mHTTP/2\x1b[0m",
            Http3 => "\x1b[34mHTTP/3\x1b[0m",
        },
    )
}

pub fn check_loglevel<N: Into<u64>>(level: N) -> bool {
    (LOGLEVEL.load(std::sync::atomic::Ordering::Relaxed) & level.into()) != 0
}

#[macro_export]
macro_rules! log_with_level {
    ($level:expr, $($arg:tt)*) => {{
        if crate::logger::check_loglevel($level) {
            println!($($arg)*);
        }
    }};
}
#[macro_export]
macro_rules! elog_with_level {
    ($level:expr, $($arg:tt)*) => {{
        if crate::logger::check_loglevel($level) {
            eprintln!($($arg)*);
        }
    }};
}

// pub fn color_line_24(text: &str, foreground: Option<(u8, u8, u8)>, background: Option<(u8, u8, u8)>, reset: bool) -> String {
//     let mut text = text.to_string();
//     if let Some((r, g, b)) = foreground {
//         text = format!("\x1b[38;2;{r};{g};{b}m{text}");
//     }
//     if let Some((r, g, b)) = background {
//         text = format!("\x1b[38;2;{r};{g};{b}m{text}");
//     }
//     if reset { text += "\x1b[0m" }

//     text
// }

// pub fn color_line_8(text: &str, foreground: Option<u8>, background: Option<u8>, reset: bool) -> String {
//     let mut text = text.to_string();
//     if let Some(c) = foreground {
//         text = format!("\x1b[38;5;{c}m{text}");
//     }
//     if let Some(c) = background {
//         text = format!("\x1b[38;5;{c}m{text}");
//     }
//     if reset { text += "\x1b[0m" }

//     text
// }

pub mod loglevels {
    pub const INIT_ERROR: u64 = 1 << 0;
    // pub const SERVER_ERROR: u64 = 1 << 1;

    pub const EXIT: u64 = 1 << 2;
    pub const CLIENT_DUMP: u64 = 1 << 3;
    pub const REQUEST: u64 = 1 << 4;
    
    pub const RESPONSE: u64 = 1 << 5;
    pub const RESPONSE_TIME: u64 = 1 << 6;

    pub const HANDLER_ERROR: u64 = 1 << 7;
    pub const TLS_UPGRADE_ERROR: u64 = 1 << 8;
    pub const CONTENT_HANDLER_ERROR: u64 = 1 << 9;
    pub const HTTP2_ERROR: u64 = 1 << 10;
    pub const HTTP2_FRAME_DUMP: u64 = 1 << 11;
}

    // Debug = 1,
    // Verbose = 2,
    // Log = 4,
    // Info = 8,
    // Dump = 16,
    // Trace = 32,
    // Init = 64,
    // Warning = 128,
    // SoftError = 256,
    // Error = 512,
    // Critical = 1024,
    // Fatal = 2048,
    // Assert = 4096,