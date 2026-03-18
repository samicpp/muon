use std::{path::Path, sync::Arc, time::Instant};

use http::shared::{HttpSocket, LibError};

use crate::{DynHttpSocket, arguments::Cli, handlers::{HttpHandler, sanitize_path}, log_with_level, logger::{check_loglevel, log_client_simple, loglevels}, servers::GenAddr, settings::Settings};



pub struct SimpleHandler {
    pub _args: Arc<Cli>,
    pub settings: Arc<Settings>,
}
#[async_trait::async_trait]
impl HttpHandler for SimpleHandler{
    async fn entry(self: Arc<Self>, mut http: DynHttpSocket, addr: GenAddr) -> Result<(), LibError> {
        let client = http.read_until_head_complete().await?;
        let now = Instant::now();
        let path = Path::new(&self.settings.content.serve_dir).join(sanitize_path(&client.path));

        let mut status = 200;

        log_with_level!(loglevels::CLIENT_DUMP, "\x1b[90m[{:?}]\x1b[0m {}", addr, log_client_simple(client));

        http.set_header("Server", "simple-serve");
        http.set_header("Content-Type", "text/plain");

        if !path.exists() {
            status = 404;
            http.set_status(404, "Not Found".to_owned());
            http.close(format!("{:?} not found", path).as_bytes()).await?;
        }
        else if path.is_file() {
            self.file_handler(&mut http, &path).await?;
        }
        else if path.is_dir() {
            let name = path.file_name().map(|s| s.to_string_lossy()).unwrap_or("index".into());

            let mut found = None;
            let mut dir = tokio::fs::read_dir(&path).await?;
            while let Some(file) = dir.next_entry().await? {
                if file.metadata().await.map(|m| m.is_file()).unwrap_or(false) && (file.file_name().to_string_lossy().starts_with(name.as_ref()) || file.file_name().to_string_lossy().starts_with("index")) {
                    found = Some(file);
                }
            }

            if let Some(found) = found {
                self.file_handler(&mut http, &found.path()).await?;
            } 
            else {
                http.set_status(404, "Not Found".to_owned());
                http.close(format!("couldnt find file in {:?}", &path).as_bytes()).await?;
            }
        }
        else {
            http.set_status(501, "Not Implemented".to_owned());
            http.close(b"couldn't handle file").await?;
        }


        log_with_level!(loglevels::RESPONSE, "{:?} {}", path, status);

        if check_loglevel(loglevels::RESPONSE_TIME) {
            let nanos = now.elapsed().as_nanos();
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

            println!("response took {}", &stamp);
        }

        Ok(())
    }
}
impl SimpleHandler {
    async fn file_handler(&self, http: &mut DynHttpSocket, path: &Path) -> Result<(), LibError> {
        let meta = path.metadata()?;
        let name = path.file_name().map(|s| s.to_string_lossy()).unwrap_or("".into());

        if let Some(max) = self.settings.content.max_file_size && meta.len() > max as u64 {
            http.set_status(503, "Service Unavailable".to_owned());
            http.close(b"file too big").await?;
        }
        else {
            if name.ends_with(".html") { http.set_header("Content-Type", "text/html"); }
            else if name.ends_with(".js") { http.set_header("Content-Type", "text/javascript"); }
            else if name.ends_with(".css") { http.set_header("Content-Type", "text/css"); }
            else if name.ends_with(".png") { http.set_header("Content-Type", "img/png"); }
            else if name.ends_with(".jpg") { http.set_header("Content-Type", "img/jpeg"); }
            else if name.ends_with(".jpeg") { http.set_header("Content-Type", "img/jpeg"); }

            http.close(&tokio::fs::read(path).await?).await?;
        }
        Ok(())
    }
}