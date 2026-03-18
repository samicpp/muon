use std::{path::Path, sync::Arc};

use http::shared::{HttpSocket, LibError};

use crate::{DynHttpSocket, arguments::Cli, handlers::{HttpHandler, sanitize_path}, servers::GenAddr, settings::Settings};



pub struct SimpleHandler {
    pub _args: Arc<Cli>,
    pub settings: Arc<Settings>,
}
#[async_trait::async_trait]
impl HttpHandler for SimpleHandler{
    async fn entry(self: Arc<Self>, mut http: DynHttpSocket, addr: GenAddr) -> Result<(), LibError> {
        let client = http.read_until_head_complete().await?;
        let path = Path::new(&self.settings.content.serve_dir).join(sanitize_path(&client.path));

        println!("[{:?}] {} {}", addr, client.method, client.path);

        http.set_header("Server", "simple-serve");
        http.set_header("Content-Type", "text/plain");

        if !path.exists() {
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