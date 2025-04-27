use std::collections::HashSet;
use std::env;
use rand::prelude::*;

/// Parses a single proxy line.
/// If no schema is present, defaults to "http://"
pub fn parse_proxy_line(line: &str) -> String {
    let trimmed = line.trim().to_lowercase();
    if trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
        || trimmed.starts_with("socks4://")
        || trimmed.starts_with("socks5://")
    {
        line.trim().to_string()
    } else {
        format!("http://{}", line.trim())
    }
}

/// Picks a random proxy not in tried_proxies.
/// If all proxies were tried, clears tried_proxies automatically and reuses the list.
pub fn pick_random_untried_proxy(
    proxy_list: &[String],
    tried_proxies: &mut HashSet<String>,
) -> Option<String> {
    let mut rng = rand::thread_rng();

    // If all proxies are tried, reset
    if tried_proxies.len() >= proxy_list.len() {
        println!("[*] All proxies tried. Resetting tried list...");
        tried_proxies.clear();
    }

    let untried: Vec<&String> = proxy_list.iter()
        .filter(|p| !tried_proxies.contains(*p))
        .collect();

    if !untried.is_empty() {
        Some(untried.choose(&mut rng).unwrap().to_string())
    } else {
        proxy_list.choose(&mut rng).cloned()
    }
}

/// Sets ALL_PROXY so all protocols (HTTP, HTTPS, SOCKS4, SOCKS5) use the proxy.
pub fn set_all_proxy_env(proxy: &str) {
    env::set_var("ALL_PROXY", proxy);
}

/// Clears all proxy-related environment variables.
pub fn clear_proxy_env_vars() {
    env::remove_var("ALL_PROXY");
    env::remove_var("HTTP_PROXY");
    env::remove_var("HTTPS_PROXY");
}
