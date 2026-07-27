use std::sync::{Arc, RwLock};

use tokio::io::AsyncBufReadExt;

use crate::{arguments::Cli, log_with_level, settings::Settings};


#[derive(Debug)]
pub struct ConTxRx(pub tokio::sync::broadcast::Sender<ConsoleCommand>, pub tokio::sync::broadcast::Receiver<ConsoleCommand>);
impl Clone for ConTxRx {
    fn clone(&self) -> Self {
        Self (self.0.clone(), self.0.subscribe())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleCommand {
    Kill,
    Stop,
    Shutdown,
    Reload,
    Restart,
}

pub async fn console(_args: Arc<Cli>, settings: Arc<RwLock<Settings>>, txrx: ConTxRx) -> std::io::Result<()> {
    let mut stdin = tokio::io::BufReader::new(tokio::io::stdin());
    while settings.read().unwrap().environment.console {
        let mut buff = String::new();
        stdin.read_line(&mut buff).await?;
        log_with_level!(false, settings.read().unwrap().logging.console_repeat, "\x1b[32m{:?}\x1b[0m", buff);
        

        if buff == "kill\n" {
            let _ = txrx.0.send(ConsoleCommand::Kill);
            log_with_level!(false, settings.read().unwrap().logging.console_operation, "stopping");
            break;
        } 
        else if buff == "stop\n" {
            let _ = txrx.0.send(ConsoleCommand::Stop);
            log_with_level!(false, settings.read().unwrap().logging.console_operation, "stopping");
            break;
        } 
        else if buff == "shutdown\n" {
            let _ = txrx.0.send(ConsoleCommand::Shutdown);
            log_with_level!(false, settings.read().unwrap().logging.console_operation, "sent shiutdown");
        }
        else if buff == "restart\n" {
            let _ = txrx.0.send(ConsoleCommand::Restart);
            log_with_level!(false, settings.read().unwrap().logging.console_operation, "stopping");
            break;
        }
        else if buff == "reload\n" {
            let _ = txrx.0.send(ConsoleCommand::Reload);
            log_with_level!(false, settings.read().unwrap().logging.console_operation, "stopping");
            break;
        }
    }
    Ok(())
}