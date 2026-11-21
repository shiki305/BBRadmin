//! Health checks for the co-op server: TCP connect and lightweight HTTP GET /status.

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

use crate::core::coop_schema;

pub fn tcp(host: &str, port: u16, timeout_ms: u64) -> bool {
    let timeout = Duration::from_millis(timeout_ms);
    let addr_str = format!("{host}:{port}");
    let Ok(addrs) = addr_str.to_socket_addrs() else {
        return false;
    };
    for addr in addrs {
        if TcpStream::connect_timeout(&addr, timeout).is_ok() {
            return true;
        }
    }
    false
}

/// Sends a minimal HTTP/1.1 GET /status to `host:port` and returns the status line (e.g. "200 OK"),
/// or "" when nothing answers.
pub fn http_status(host: &str, port: u16, path: &str, timeout_ms: u64) -> String {
    let timeout = Duration::from_millis(timeout_ms);
    let addr_str = format!("{host}:{port}");
    let addrs: Vec<SocketAddr> = match addr_str.to_socket_addrs() {
        Ok(it) => it.collect(),
        Err(_) => return String::new(),
    };
    for addr in addrs {
        if let Ok(mut stream) = TcpStream::connect_timeout(&addr, timeout) {
            let _ = stream.set_read_timeout(Some(timeout));
            let _ = stream.set_write_timeout(Some(timeout));
            let req = format!(
                "GET {path} HTTP/1.1\r\nHost: {host}:{port}\r\nConnection: close\r\n\r\n"
            );
            if stream.write_all(req.as_bytes()).is_err() {
                continue;
            }
            let mut reader = BufReader::new(stream);
            let mut first_line = String::new();
            if reader.read_line(&mut first_line).is_ok() && !first_line.trim().is_empty() {
                let trimmed = first_line.trim();
                // Strip "HTTP/1.x " prefix -> "200 OK"
                if let Some((_, rest)) = trimmed.split_once(' ') {
                    return rest.trim().to_string();
                }
                return trimmed.to_string();
            }
        }
    }
    String::new()
}

pub fn reachable(host_ip: &str, timeout_ms: u64) -> bool {
    if !http_status(host_ip, coop_schema::WEBAPI_PORT, "/status", timeout_ms).is_empty() {
        return true;
    }
    tcp(host_ip, coop_schema::BACKEND_PORT, timeout_ms)
}

pub fn describe(host_ip: &str, timeout_ms: u64) -> String {
    describe_for(host_ip, timeout_ms, crate::core::i18n::Lang::PtBr)
}

pub fn describe_for(host_ip: &str, timeout_ms: u64, lang: crate::core::i18n::Lang) -> String {
    use crate::core::i18n::t;
    let status = http_status(host_ip, coop_schema::WEBAPI_PORT, "/status", timeout_ms);
    let webapi = if !status.is_empty() {
        format!("{} ({status})", t(lang, "responde", "online"))
    } else {
        t(lang, "sem resposta", "no response").to_string()
    };
    let backend = if tcp(host_ip, coop_schema::BACKEND_PORT, timeout_ms) {
        t(lang, "responde", "online")
    } else {
        t(lang, "sem resposta", "no response")
    };
    format!(
        "WebAPI {}: {} · backend {}: {} · link UDP {}",
        coop_schema::WEBAPI_PORT,
        webapi,
        coop_schema::BACKEND_PORT,
        backend,
        coop_schema::LINK_PORT
    )
}
