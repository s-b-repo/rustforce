
# Rustforce

> A **fast**, **modular**, and **powerful** brute-forcing toolkit written in Rust, targeting **FTP**, **SSH**, and **Telnet** services — bundled with two Python utilities for **IP generation** and **proxy downloading/testing**.

---
## 🛠 Features

- 🚀 **Rust-based Brute-Forcer**:
  - FTP, SSH, and Telnet brute-force modules.
  - Highly concurrent (async-based using `tokio`).
  - Proxy support (HTTP/SOCKS4/SOCKS5).
  - Auto-retry and timeout handling.
  - Optimized for huge wordlists.
  - Supports IPv4 and proxies simultaneously.

- 🐍 **Python Utilities**:
  - **IP Generator**: Generates and saves all possible public IPv4 addresses.
  - **Proxy Manager**:
    - Downloads proxies from multiple sources.
    - Tests proxies against real targets (e.g., DuckDuckGo, Bing).
    - Saves working proxies into a clean, ready-to-use format.

- 📦 **Organized Output**:
  - Save successful brute-force results automatically.
  - Logs failed, timeout, and success events.

---
## 📂 Project Structure

```
RustBruteSuite/
├── src/
│   ├── ftp.rs        # FTP Brute-forcer
│   ├── ssh.rs        # SSH Brute-forcer
│   ├── telnet.rs     # Telnet Brute-forcer
│   ├── proxy.rs      # Proxy management
│   ├── events.rs     # Event system (Success/Fail/Timeout)
│   └── main.rs       # Entry point
├── Cargo.toml    # Rust project manifest
├── README.md     # You are here
└── LICENSE    
```

---

## 🚀 Getting Started

### Prerequisites


- pip install requests
- Rust (>= 1.70)
- Python 3.10+ (for Python utilities)

### Install Rust (if you don't have it)

```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

---

## 🦀 Building & Running the Rust Brute-Forcer

Clone the repository:

```
git clone https://github.com/yourusername/RustBruteSuite.git
cd RustBruteSuite
```

Build the Rust project:

```
cargo build --release
```

Run it:

```
cargo run --release
```

You will be prompted for:
- Target file (list of IPs)
- Username wordlist
- Password wordlist
- Proxy file (optional)

### Example:

```
cargo run --release
```
> [*] Loading targets from `ips.txt`
> [*] Using `usernames.txt` and `passwords.txt`
> [*] Loaded 5000 proxies from `proxies.txt`
> [*] Starting FTP, SSH, and Telnet brute-forcing...

---

## 🐍 Using the Python Tools

### 1. IPv4 Generator

Generates **every possible public IPv4 address** (avoiding private/reserved ranges) and saves to a file.

```
cd python_tools
python3 generate_ipv4.py
```

- Output: `ips.txt`
- About 3.7 billion addresses (⚡ Super fast, multi-threaded)

---

### 2. Proxy Downloader, Tester, and Saver

Downloads proxies from multiple sources, tests them against real websites (e.g., DuckDuckGo and Bing), and saves working ones.

```
cd python_tools
python3 proxy_downloader.py
```

- Output: `proxies.txt`
- Proxy format: `socks5://IP:PORT`, `http://IP:PORT`, etc.

---

## ⚙️ Configuration

Edit `src/main.rs` to adjust:

| Option | Description | Default |
|:------:|:------------|:-------:|
| `MAX_CONCURRENT_IPS` | How many IPs to attack in parallel | 10 |
| `CONNECT_TIMEOUT_SECS` | Connection timeout in seconds | 4 |
| `RETRY_DELAY_MS` | Retry delay between attempts | 100 |
| `MAX_RETRIES` | Max retries per IP/protocol | 2 |

---

## 📈 Performance Tips

- Use smaller proxy lists (only fast-working proxies).
- Use a VPS/server close to target region.
- Tweak `MAX_CONCURRENT_IPS` carefully (too high = IP ban).
- Run multiple instances targeting different IP ranges.

---

## ❗ Legal Disclaimer

> This project is **for educational and authorized penetration testing purposes only**.  
> Unauthorized access to systems without permission is illegal and punishable by law.  
> The authors of RustBruteSuite are **not responsible** for any misuse or damage caused by this tool.

---

---

## 📜 License

This project is licensed under the [MIT License](LICENSE).

---

## 💬 Credits

Built with 💻 by [Your Name or Organization].

---
  
Would you also want me to give you **badges** (e.g., "Made with Rust", "Python3 Utilities", "MIT License", "Brute-force toolkit") and maybe a **cool ASCII art logo** for the README too? 🚀  
It can make the GitHub page look even more badass!  
Want me to generate that too? 🎨✨
