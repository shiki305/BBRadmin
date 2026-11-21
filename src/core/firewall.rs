//! Windows Firewall management for the host via netsh advfirewall (VPN + LocalSubnet only).

use std::os::windows::process::CommandExt;
use std::process::Command;

use crate::core::coop_schema;
use crate::core::log;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
pub const REMOTE_RANGES: &str = "26.0.0.0/8,25.0.0.0/8,100.64.0.0/10,LocalSubnet";

pub struct FirewallRule {
    pub name: &'static str,
    pub protocol: &'static str,
    pub port: u16,
}

pub const RULES: &[FirewallRule] = &[
    FirewallRule {
        name: "BBRadmin link 31314",
        protocol: "UDP",
        port: coop_schema::LINK_PORT,
    },
    FirewallRule {
        name: "BBRadmin webapi 31315",
        protocol: "TCP",
        port: coop_schema::WEBAPI_PORT,
    },
    FirewallRule {
        name: "BBRadmin game server 20443",
        protocol: "TCP",
        port: coop_schema::BACKEND_PORT,
    },
];

pub fn installed(rule: &FirewallRule) -> bool {
    let output = run_netsh(&[
        "advfirewall",
        "firewall",
        "show",
        "rule",
        &format!("name={}", rule.name),
    ]);
    output.contains(&rule.port.to_string()) && output.contains("26.0.0.0/8")
}

pub fn missing() -> Vec<&'static str> {
    RULES
        .iter()
        .filter(|r| !installed(r))
        .map(|r| r.name)
        .collect()
}

pub fn install_all() -> (bool, String) {
    let mut text = String::new();
    let mut ok = true;
    for rule in RULES {
        let _ = run_netsh(&[
            "advfirewall",
            "firewall",
            "delete",
            "rule",
            &format!("name={}", rule.name),
        ]);
        let output = run_netsh(&[
            "advfirewall",
            "firewall",
            "add",
            "rule",
            &format!("name={}", rule.name),
            "dir=in",
            "action=allow",
            &format!("protocol={}", rule.protocol),
            &format!("localport={}", rule.port),
            &format!("remoteip={REMOTE_RANGES}"),
        ]);
        let made = installed(rule);
        if !made {
            ok = false;
        }
        text.push_str(&format!(
            "{} {} ({} {})\n",
            if made { "ok   " } else { "FALHA" },
            rule.name,
            rule.protocol,
            rule.port
        ));
        if !made && !output.trim().is_empty() {
            text.push_str(&format!(
                "      {}\n",
                output.replace("\r\n", " ").replace('\n', " ").trim()
            ));
        }
    }
    (ok, text)
}

pub fn manual_commands() -> String {
    let mut text = String::new();
    for rule in RULES {
        text.push_str(&format!(
            "netsh advfirewall firewall add rule name=\"{}\" dir=in action=allow protocol={} localport={} remoteip={}\n",
            rule.name, rule.protocol, rule.port, REMOTE_RANGES
        ));
    }
    text
}

fn run_netsh(args: &[&str]) -> String {
    match Command::new("netsh.exe")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    {
        Ok(out) => {
            let mut s = String::from_utf8_lossy(&out.stdout).into_owned();
            s.push_str(&String::from_utf8_lossy(&out.stderr));
            s
        }
        Err(err) => {
            log::line(&format!("netsh falhou ({err})."));
            String::new()
        }
    }
}
