#!/usr/bin/env python3
import requests
import re
import time
import threading
from concurrent.futures import ThreadPoolExecutor, as_completed
from typing import Set, Dict

# ─── 1) SOURCES GROUPED BY PROTOCOL ───────────────────────────────────────────
sources: Dict[str, list[str]] = {
    'socks4': [
        'https://raw.githubusercontent.com/SevenworksDev/proxy-list/main/proxies/socks4.txt',
        'https://raw.githubusercontent.com/ErcinDedeoglu/proxies/main/proxies/socks4.txt',
        'https://raw.githubusercontent.com/casals-ar/proxy-list/main/socks4',  # <-- corrected
    ],
    'socks5': [
        'https://raw.githubusercontent.com/SevenworksDev/proxy-list/main/proxies/socks5.txt',
        'https://raw.githubusercontent.com/ErcinDedeoglu/proxies/main/proxies/socks5.txt',
        'https://raw.githubusercontent.com/casals-ar/proxy-list/main/socks5',  # <-- corrected
    ],
    'http': [
        'https://raw.githubusercontent.com/SevenworksDev/proxy-list/main/proxies/http.txt',
        'https://raw.githubusercontent.com/ErcinDedeoglu/proxies/main/proxies/http.txt',
        'https://raw.githubusercontent.com/casals-ar/proxy-list/main/http',     # <-- corrected
    ],
    'https': [
        'https://raw.githubusercontent.com/SevenworksDev/proxy-list/main/proxies/https.txt',
        'https://raw.githubusercontent.com/ErcinDedeoglu/proxies/main/proxies/https.txt',
    ]
}

# ─── 2) CONSTANTS ─────────────────────────────────────────────────────────────
IP_PORT_RE = re.compile(r'^\d{1,3}(?:\.\d{1,3}){3}:\d{1,5}$')
save_lock = threading.Lock()
HEADERS = {
    'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 '
                  '(KHTML, like Gecko) Chrome/113.0.0.0 Safari/537.36'
}
FETCH_THREADS = 16
TEST_THREADS = 120
REQUEST_TIMEOUT = 5

# ─── FETCH SOURCES ─────────────────────────────────────────────────────────────
def fetch_source(url: str, proto: str) -> Set[str]:
    try:
        resp = requests.get(url, timeout=10)
        resp.raise_for_status()
    except Exception as e:
        print(f"[!] Failed to fetch {url}: {e}")
        return set()

    proxies = set()
    for line in resp.text.splitlines():
        line = line.strip()
        if not line or line.startswith('#'):
            continue
        if '://' in line:
            _, rest = line.split('://', 1)
        else:
            rest = line
        rest = rest.strip()
        if IP_PORT_RE.match(rest):
            proxies.add(f"{proto}://{rest}")
    return proxies

def fetch_proxies() -> Set[str]:
    all_proxies = set()
    with ThreadPoolExecutor(max_workers=FETCH_THREADS) as pool:
        futures = []
        for proto, urls in sources.items():
            for url in urls:
                futures.append(pool.submit(fetch_source, url, proto))
        for fut in as_completed(futures):
            all_proxies.update(fut.result())
    return all_proxies




# ─── 4) TESTING PROXIES ───────────────────────────────────────────────────────
def test_proxy(proxy: str, timeout: int = REQUEST_TIMEOUT) -> bool:
    test_urls = [
        ('http://example.com', 'Example Domain'),
        ('http://neverssl.com', 'NeverSSL'),
        ('https://duckduckgo.com', 'DuckDuckGo'),
        ('https://www.bing.com', 'Bing'),
        ('https://httpbin.org/ip', None),
    ]

    proto, addr = proxy.split('://', 1)
    print(f"\n[>] Testing proxy {proxy} ({proto.upper()})")

    session = requests.Session()
    session.headers.update(HEADERS)

    if proto in ['socks4', 'socks5']:
        session.proxies.update({
            'http': proxy,
            'https': proxy,
        })
    elif proto in ['http', 'https']:
        session.proxies.update({
            'http': proxy,
            'https': proxy,
        })
    else:
        print(f"[!] Unknown proxy type: {proto}")
        return False

    success = False

    for url, keyword in test_urls:
        try:
            print(f"[*] Trying {url} via {proxy}")
            r = session.get(url, timeout=timeout)

            if r.status_code == 200:
                if keyword:
                    if keyword.lower() in r.text.lower():
                        print(f"[+] Found keyword '{keyword}'! Good proxy.")
                        success = True
                        break
                    else:
                        print(f"[-] Keyword '{keyword}' missing, trying next...")
                else:
                    print(f"[+] 200 OK! (no keyword needed)")
                    success = True
                    break
            else:
                print(f"[-] Status {r.status_code}")

        except (requests.exceptions.ProxyError, requests.exceptions.SSLError,
                requests.exceptions.ConnectTimeout, requests.exceptions.ReadTimeout,
                requests.exceptions.ConnectionError) as e:
            print(f"[!] Connection failed: {e}")
        except Exception as e:
            print(f"[!] Unexpected error: {type(e).__name__}: {e}")

    if success:
        save_proxy_immediate(proxy)
    else:
        print(f"[✖] Proxy {proxy} failed all tests.")
    return success

# ─── SAVE GOOD PROXIES ─────────────────────────────────────────────────────────
def save_proxy_immediate(proxy: str):
    proto = proxy.split('://', 1)[0]
    filename = f"{proto}.txt"
    with save_lock:
        with open(filename, 'a') as f:
            f.write(proxy + '\n')
    print(f"[💾] Saved {proxy} to {filename}")


# ─── TEST ALL PROXIES ──────────────────────────────────────────────────────────
def test_proxies(cands: Set[str]):
    print(f"[*] Testing {len(cands)} proxies with {TEST_THREADS} threads...")
    with ThreadPoolExecutor(max_workers=TEST_THREADS) as pool:
        future_map = {pool.submit(test_proxy, p): p for p in cands}
        done = 0
        for fut in as_completed(future_map):
            proxy = future_map[fut]
            try:
                if fut.result():
                    print(f"[+] OK: {proxy}")
                else:
                    print(f"[-] FAIL: {proxy}")
            except Exception as e:
                print(f"[!] Exception testing {proxy}: {e}")
            done += 1
            if done % 10 == 0:
                print(f"[{done}/{len(cands)}] tested...")

# ─── MAIN ──────────────────────────────────────────────────────────────────────
def main():
    print("[*] Fetching proxy lists...")
    candidates = fetch_proxies()
    if not candidates:
        print("⚠ No proxies fetched. Check your sources or network.")
        return

    print(f"[*] {len(candidates)} proxies fetched. Starting tests...")
    test_proxies(candidates)
    print("\n[✔] All done!")

if __name__ == '__main__':
    main()
