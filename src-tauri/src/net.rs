use std::net::IpAddr;

pub fn list_physical_nics() -> Vec<(String, String)> {
    use network_interface::{NetworkInterface, NetworkInterfaceConfig};
    let mut out: Vec<(String, String)> = Vec::new();
    if let Ok(nics) = NetworkInterface::show() {
        for nic in nics {
            for addr in &nic.addr {
                let ip = match addr.ip() {
                    IpAddr::V4(v4) => v4.to_string(),
                    IpAddr::V6(_) => continue,
                };
                if addr.ip().is_loopback() { continue; }
                if is_virtual_interface(&nic.name) { continue; }
                out.push((nic.name.clone(), ip));
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

pub fn is_virtual_interface(name: &str) -> bool {
    let n = name.to_lowercase();
    const KEYWORDS: &[&str] = &[
        "wsl", "docker", "vethernet", "hyper-v", "hyperv",
        "virtualbox", "vmware", "vpn", "zerotier", "tailscale",
        "isatap", "teredo", "bluestacks", "npcap", "openvpn",
    ];
    KEYWORDS.iter().any(|k| n.contains(k))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_is_virtual_wsl() {
        assert!(is_virtual_interface("vEthernet (WSL)"));
        assert!(is_virtual_interface("WSL"));
    }
    #[test]
    fn test_is_virtual_docker() {
        assert!(is_virtual_interface("docker0"));
        assert!(is_virtual_interface("vEthernet (Docker)"));
    }
    #[test]
    fn test_is_virtual_vpn() {
        assert!(is_virtual_interface("VPN VPN"));
    }
    #[test]
    fn test_is_physical() {
        assert!(!is_virtual_interface("Ethernet"));
        assert!(!is_virtual_interface("Wi-Fi"));
    }
}
