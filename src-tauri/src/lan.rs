//! LAN address policy: the app only talks to private addresses on a subnet it is attached to.

use std::net::Ipv4Addr;

/// RFC 1918, not loopback/link-local/broadcast, and inside the `iface`/`prefix_len` subnet.
pub fn is_allowed_lan_ip(ip: Ipv4Addr, iface: Ipv4Addr, prefix_len: u8) -> bool {
    if !(8..=32).contains(&prefix_len) || !ip.is_private() || ip.is_loopback() || ip.is_link_local()
    {
        return false;
    }
    let mask = u32::MAX
        .checked_shl(32 - u32::from(prefix_len))
        .unwrap_or(0);
    let (ip, iface) = (u32::from(ip), u32::from(iface));
    // Same subnet; up to /30 also not the network or broadcast address (/31 and /32 have neither).
    ip & mask == iface & mask && (prefix_len > 30 || (ip & !mask != 0 && ip | mask != u32::MAX))
}

/// Whether `ip` is allowed on any of the Mac's own IPv4 interfaces.
pub fn on_local_subnet(ip: Ipv4Addr) -> bool {
    if_addrs::get_if_addrs().is_ok_and(|ifaces| {
        ifaces.iter().any(|i| match &i.addr {
            if_addrs::IfAddr::V4(a) => is_allowed_lan_ip(
                ip,
                a.ip,
                u8::try_from(u32::from(a.netmask).leading_ones()).unwrap_or(0),
            ),
            if_addrs::IfAddr::V6(_) => false,
        })
    })
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
    fn rejects_implausible_prefixes() {
        let iface = ip(192, 168, 1, 156);
        assert!(!is_allowed_lan_ip(ip(192, 168, 1, 10), iface, 0));
        assert!(!is_allowed_lan_ip(ip(192, 168, 1, 10), iface, 7));
        assert!(!is_allowed_lan_ip(ip(192, 168, 1, 10), iface, 33));
        assert!(is_allowed_lan_ip(ip(10, 0, 0, 5), ip(10, 0, 0, 2), 8));
    }

    #[test]
    fn rejects_the_network_address() {
        assert!(!is_allowed_lan_ip(
            ip(192, 168, 1, 0),
            ip(192, 168, 1, 156),
            24
        ));
        assert!(!is_allowed_lan_ip(
            ip(192, 168, 16, 0),
            ip(192, 168, 17, 5),
            20
        ));
        assert!(is_allowed_lan_ip(
            ip(192, 168, 17, 0),
            ip(192, 168, 17, 5),
            20
        ));
    }

    #[test]
    fn point_to_point_and_host_prefixes() {
        // /31 has no network or broadcast address; /32 is the interface itself.
        assert!(is_allowed_lan_ip(
            ip(192, 168, 1, 4),
            ip(192, 168, 1, 5),
            31
        ));
        assert!(is_allowed_lan_ip(
            ip(192, 168, 1, 5),
            ip(192, 168, 1, 5),
            31
        ));
        assert!(!is_allowed_lan_ip(
            ip(192, 168, 1, 6),
            ip(192, 168, 1, 5),
            31
        ));
        assert!(is_allowed_lan_ip(
            ip(192, 168, 1, 5),
            ip(192, 168, 1, 5),
            32
        ));
        assert!(!is_allowed_lan_ip(
            ip(192, 168, 1, 6),
            ip(192, 168, 1, 5),
            32
        ));
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
