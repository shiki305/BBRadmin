//! Detects this PC's IPv4 addresses via Windows GetAdaptersAddresses, ranking Radmin VPN (26/8) first.

use std::net::Ipv4Addr;
use std::ptr;

use windows_sys::Win32::Foundation::{ERROR_BUFFER_OVERFLOW, NO_ERROR};
use windows_sys::Win32::NetworkManagement::IpHelper::{
    GetAdaptersAddresses, GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER,
    GAA_FLAG_SKIP_MULTICAST, IF_TYPE_SOFTWARE_LOOPBACK, IP_ADAPTER_ADDRESSES_LH,
};
use windows_sys::Win32::Networking::WinSock::{AF_INET, SOCKADDR_IN};

const IF_OPER_STATUS_UP: i32 = 1;

#[derive(Debug, Clone)]
pub struct NetworkChoice {
    pub adapter: String,
    pub ip: String,
    pub kind: String,
}

impl NetworkChoice {
    pub fn describe(&self) -> String {
        format!("{} · {} · {}", self.kind, self.ip, self.adapter)
    }
}

pub fn detect() -> Vec<NetworkChoice> {
    let flags = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER;
    let mut size = 16 * 1024u32;
    let mut buf = vec![0u8; size as usize];

    for _ in 0..3 {
        let ret = unsafe {
            GetAdaptersAddresses(
                AF_INET as u32,
                flags,
                ptr::null_mut(),
                buf.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH,
                &mut size,
            )
        };
        if ret == ERROR_BUFFER_OVERFLOW {
            buf.resize(size as usize, 0);
            continue;
        }
        if ret != NO_ERROR {
            return Vec::new();
        }
        break;
    }

    let mut found = Vec::new();
    let mut adapter_ptr = buf.as_ptr() as *const IP_ADAPTER_ADDRESSES_LH;
    while !adapter_ptr.is_null() {
        let adapter = unsafe { &*adapter_ptr };
        if adapter.OperStatus == IF_OPER_STATUS_UP && adapter.IfType != IF_TYPE_SOFTWARE_LOOPBACK {
            let name = pwstr_to_string(adapter.FriendlyName);
            let mut uni_ptr = adapter.FirstUnicastAddress;
            while !uni_ptr.is_null() {
                let uni = unsafe { &*uni_ptr };
                let sa = uni.Address.lpSockaddr;
                if !sa.is_null() && unsafe { (*sa).sa_family } == AF_INET {
                    let sin = unsafe { &*(sa as *const SOCKADDR_IN) };
                    let octets = unsafe { sin.sin_addr.S_un.S_un_b };
                    let ip = Ipv4Addr::new(octets.s_b1, octets.s_b2, octets.s_b3, octets.s_b4);
                    let ip_str = ip.to_string();
                    if !ip_str.starts_with("169.254.") {
                        let kind = kind_of(&ip_str, &name);
                        found.push(NetworkChoice {
                            adapter: name.clone(),
                            ip: ip_str,
                            kind: kind.to_string(),
                        });
                    }
                }
                uni_ptr = uni.Next;
            }
        }
        adapter_ptr = adapter.Next;
    }

    found.sort_by(|a, b| rank(&a.kind).cmp(&rank(&b.kind)).then_with(|| a.ip.cmp(&b.ip)));
    found
}

pub fn pick(all: &[NetworkChoice]) -> Option<&NetworkChoice> {
    all.first()
}

pub fn is_vpn(kind: &str) -> bool {
    !kind.is_empty() && kind != "Rede local" && kind != "Outra rede"
}

pub fn kind_of_ip_only(ip: &str) -> &'static str {
    kind_of(ip, "")
}

fn rank(kind: &str) -> u8 {
    match kind {
        "Radmin VPN" => 0,
        "Hamachi" => 1,
        "Tailscale" => 2,
        "ZeroTier" => 3,
        "Rede local" => 4,
        _ => 5,
    }
}

fn kind_of(ip: &str, adapter: &str) -> &'static str {
    let parts: Vec<u8> = ip.split('.').filter_map(|p| p.parse::<u8>().ok()).collect();
    let (first, second) = if parts.len() == 4 {
        (parts[0], parts[1])
    } else {
        (0, 0)
    };
    let both = format!("{adapter} {ip}").to_ascii_lowercase();
    if both.contains("radmin") || first == 26 {
        return "Radmin VPN";
    }
    if both.contains("hamachi") || first == 25 {
        return "Hamachi";
    }
    if both.contains("tailscale") || (first == 100 && (64..=127).contains(&second)) {
        return "Tailscale";
    }
    if both.contains("zerotier") {
        return "ZeroTier";
    }
    if first == 10 || first == 172 || first == 192 {
        return "Rede local";
    }
    "Outra rede"
}

fn pwstr_to_string(ptr: *const u16) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let mut len = 0usize;
    unsafe {
        while *ptr.add(len) != 0 {
            len += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len))
    }
}
