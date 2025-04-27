use std::collections::HashSet;
use rand::prelude::*;
use anyhow::Result;
use reqwest::{Client, Proxy};
use rand::thread_rng;

/// Parses a single proxy line.
/// If no scheme is present, defaults to "http://"
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

/// Picks a random untried proxy.
/// If all proxies have been tried, it clears the tried list automatically.
pub fn pick_random_untried_proxy(
    proxy_list: &[String],
    tried_proxies: &mut HashSet<String>,
) -> Option<String> {
    let mut rng = thread_rng();

    if proxy_list.is_empty() {
        return None;
    }

    if tried_proxies.len() >= proxy_list.len() {
        println!("[*] All proxies have been tried. Resetting tried proxies...");
        tried_proxies.clear();
    }

    let untried: Vec<&String> = proxy_list
        .iter()
        .filter(|p| !tried_proxies.contains(*p))
        .collect();

    if !untried.is_empty() {
        Some(untried.choose(&mut rng).unwrap().to_string())
    } else {
        proxy_list.choose(&mut rng).cloned()
    }
}

/// Builds a reqwest::Client that uses the provided proxy for HTTP/HTTPS.
/// Works for proxy types: socks4, socks5, http, https.
pub fn build_proxy_client(proxy_url: &str) -> Result<Client> {
    Ok(Client::builder()
        .proxy(Proxy::all(proxy_url)?)
        .danger_accept_invalid_certs(true) // Accept invalid SSL certs to avoid random hangs
        .build()?)
}

/// Builds a direct reqwest::Client with no proxy.
pub fn build_direct_client() -> Result<Client> {
    Ok(Client::builder()
        .danger_accept_invalid_certs(true)
        .build()?)
}
