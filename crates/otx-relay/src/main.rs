//! otx-relay: run an OTX Relay.
//!
//!   otx-relay --listen 0.0.0.0:51888 --clients clients.txt --allow allow.txt
//!
//! clients.txt: one "client_id_hex token" per line (give each player their own pair).
//! allow.txt:   one IPv4 CIDR per line: the game-server ranges this relay may reach.
//! Optional: --pps 2000 --bps 2000000 (per-client limits). Nothing about traffic content is logged.

use otx_relay::server::{Config, Relay};
use otx_relay::Cidr;
use std::collections::HashMap;

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

fn read_lines(path: &str) -> Result<Vec<String>, String> {
    Ok(std::fs::read_to_string(path)
        .map_err(|e| format!("{path}: {e}"))?
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect())
}

fn config(args: &[String]) -> Result<Config, String> {
    let listen = arg(args, "--listen").unwrap_or_else(|| "0.0.0.0:51888".into()).parse().map_err(|e| format!("--listen: {e}"))?;
    let mut clients = HashMap::new();
    for l in read_lines(&arg(args, "--clients").ok_or("--clients <file> is required")?)? {
        let (id, tok) = l.split_once(char::is_whitespace).ok_or_else(|| format!("bad client line: {l}"))?;
        let id = u64::from_str_radix(id.trim(), 16).map_err(|_| format!("client id must be hex: {id}"))?;
        let tok = tok.trim();
        if tok.len() < 16 {
            return Err("tokens must be at least 16 characters".into());
        }
        clients.insert(id, tok.as_bytes().to_vec());
    }
    let allow: Vec<Cidr> = read_lines(&arg(args, "--allow").ok_or("--allow <file> is required (game-server ranges)")?)?
        .iter()
        .map(|l| Cidr::parse(l).ok_or_else(|| format!("bad CIDR: {l}")))
        .collect::<Result<_, _>>()?;
    if allow.is_empty() {
        return Err("the allow-list is empty; refusing to start (that would forward nothing)".into());
    }
    let pps = arg(args, "--pps").and_then(|v| v.parse().ok()).unwrap_or(2000.0);
    let bps = arg(args, "--bps").and_then(|v| v.parse().ok()).unwrap_or(2_000_000.0);
    Ok(Config { listen, clients, allow, packets_per_s: pps, bytes_per_s: bps })
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cfg = match config(&args) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("otx-relay: {e}");
            std::process::exit(2);
        }
    };
    println!("OTX Relay listening on {} for {} client(s), {} allowed range(s)", cfg.listen, cfg.clients.len(), cfg.allow.len());
    let relay = match Relay::bind(cfg).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("otx-relay: cannot bind: {e}");
            std::process::exit(1);
        }
    };
    tokio::select! {
        r = relay.run() => if let Err(e) = r { eprintln!("otx-relay: {e}"); },
        _ = tokio::signal::ctrl_c() => println!("stopping"),
    }
}
