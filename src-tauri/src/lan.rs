//! LAN address policy: the app only talks to private addresses on a subnet it is attached to.

use std::net::Ipv4Addr;

/// RFC 1918, not loopback/link-local/broadcast, and inside the `iface`/`prefix_len` subnet.
pub fn is_allowed_lan_ip(ip: Ipv4Addr, iface: Ipv4Addr, prefix_len: u8) -> bool {
    if prefix_len > 32 || !ip.is_private() || ip.is_loopback() || ip.is_link_local() {
        return false;
    }
    let mask = u32::MAX
        .checked_shl(32 - u32::from(prefix_len))
        .unwrap_or(0);
    let (ip, iface) = (u32::from(ip), u32::from(iface));
    // Same subnet, and not the subnet's own broadcast address.
    ip & mask == iface & mask && ip | mask != u32::MAX
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn ip(a: u8, b: u8, c: u8, d: u8) -> Ipv4Addr {
        Ipv4Addr::new(a, b, c, d)
    }

    #[test]
    fn rejects_public_and_foreign_subnet_ips() {
        let iface = ip(192, 168, 1, 156);
        assert!(!is_allowed_lan_ip(ip(8, 8, 8, 8), iface, 24));
        assert!(!is_allowed_lan_ip(ip(192, 168, 2, 10), iface, 24));
        assert!(!is_allowed_lan_ip(ip(127, 0, 0, 1), ip(127, 0, 0, 2), 8));
        assert!(!is_allowed_lan_ip(
            ip(169, 254, 1, 1),
            ip(169, 254, 1, 2),
            16
        ));
        assert!(!is_allowed_lan_ip(ip(192, 168, 1, 255), iface, 24));
    }

    #[test]
    fn accepts_same_subnet_private_ip() {
        assert!(is_allowed_lan_ip(
            ip(192, 168, 1, 159),
            ip(192, 168, 1, 156),
            24
        ));
        assert!(is_allowed_lan_ip(ip(10, 0, 0, 5), ip(10, 0, 0, 2), 8));
    }
}
