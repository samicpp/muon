use std::sync::Arc;

use photon::shared::{ReadStream, WriteStream};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::{arguments::Cli, log_with_level, servers::create_socket, settings::Settings};


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

pub async fn console(_args: Arc<Cli>, settings: Arc<Settings>, mut txrx: ConTxRx) -> std::io::Result<()> {
    
    if let Some(addr) = &settings.environment.console_address {
        let backlog = settings.network.backlog.unwrap_or(1024);
        let list = create_socket(addr, backlog, &settings.network)?;
        loop {
            tokio::select! {
                sock = list.accept() => {
                    let (sock, addr) = sock?;
                    log_with_level!(false, settings.logging.console_operation, "{:?} connected to console", addr);

                    let (r,w) = tokio::io::split(sock);
                    let r = BufReader::new(r);
                    let _ = handler(r, w, txrx.clone(), settings.clone()).await;
                },
                
                command = txrx.1.recv() => {
                    match command {
                        Ok(cmd) => {
                            match cmd {
                                ConsoleCommand::Shutdown |
                                ConsoleCommand::Restart |
                                ConsoleCommand::Kill |
                                ConsoleCommand::Stop => break,
                                ConsoleCommand::Reload => {},
                            }
                        },
                        Err(_) => { },
                    }
                }
            }
        }
    }
    else {
        let stdin = tokio::io::BufReader::new(tokio::io::stdin());
        handler(stdin, tokio::io::stdout(), txrx.clone(), settings.clone()).await?;
    };
    // let Some(prot) = pl.next() else { continue; };
    // let Some(loc) = pl.next() else {
    //     elog_with_level!(true, settings.logging.init_error, "invalid address: \"{}\"", binding.address);
    //     continue;
    // };

    
    Ok(())
}

pub async fn handler<R: ReadStream, W: WriteStream>(mut read: BufReader<R>, mut write: W, mut txrx: ConTxRx, settings: Arc<Settings>) -> std::io::Result<()> {
    while settings.environment.console {
        let mut buff = String::new();
        
        tokio::select! {
            res = read.read_line(&mut buff) => { let _ = res?; },
            
            command = txrx.1.recv() => {
                match command {
                    Ok(cmd) => {
                        match cmd {
                            ConsoleCommand::Shutdown |
                            ConsoleCommand::Restart |
                            ConsoleCommand::Kill |
                            ConsoleCommand::Stop => break,
                            ConsoleCommand::Reload => {},
                        }
                    },
                    Err(_) => { },
                }
            }
        }

        if settings.logging.console_repeat.unwrap_or(false) {
            println!("console: \x1b[32m{:?}\x1b[0m", buff);
            write.write(format!("\x1b[32m{:?}\x1b[0m\n", buff).as_bytes()).await?;
        }
        

        if buff == "kill\n" {
            let _ = txrx.0.send(ConsoleCommand::Kill);
            log_with_level!(false, settings.logging.console_operation, "stopping");
            break;
        } 
        else if buff == "stop\n" {
            let _ = txrx.0.send(ConsoleCommand::Stop);
            log_with_level!(false, settings.logging.console_operation, "stopping");
            break;
        } 
        else if buff == "shutdown\n" {
            let _ = txrx.0.send(ConsoleCommand::Shutdown);
            log_with_level!(false, settings.logging.console_operation, "sent shiutdown");
        }
        else if buff == "restart\n" {
            let _ = txrx.0.send(ConsoleCommand::Restart);
            log_with_level!(false, settings.logging.console_operation, "restarting");
            break;
        }
        else if buff == "reload\n" {
            let _ = txrx.0.send(ConsoleCommand::Reload);
            log_with_level!(false, settings.logging.console_operation, "reloading");
            // break;
        }
    }

    Ok(())
}