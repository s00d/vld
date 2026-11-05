//! Network formats: email, URL/URI, IP, CIDR, MAC, hostname.

use std::net::{Ipv4Addr, Ipv6Addr};

pub(crate) fn is_valid_email(s: &str) -> bool {
    // local@domain — basic RFC-like check
    let Some((local, domain)) = s.rsplit_once('@') else {
        return false;
    };
    if local.is_empty() || local.starts_with('.') || local.ends_with('.') || local.contains("..") {
        return false;
    }
    // Local part: printable ASCII except some specials, no spaces
    local
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || "!#$%&'*+/=?^_`{|}~.-".contains(ch))
        && is_valid_hostname(domain)
}

pub(crate) fn is_valid_url(s: &str) -> bool {
    let rest = match s
        .strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"))
    {
        Some(r) => r,
        None => return false,
    };
    !rest.is_empty() && !rest.contains(char::is_whitespace)
}

#[cfg(feature = "string-advanced")]
pub(crate) fn is_public_ipv4(v4: Ipv4Addr) -> bool {
    !(v4.is_private()
        || v4.is_loopback()
        || v4.is_link_local()
        || v4.is_unspecified()
        || v4.is_broadcast())
}

#[cfg(feature = "string-advanced")]
pub(crate) fn is_valid_url_strict(s: &str) -> bool {
    let parsed = match url::Url::parse(s) {
        Ok(v) => v,
        Err(_) => return false,
    };
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return false;
    }
    match parsed.host() {
        None => false,
        Some(url::Host::Domain(d)) => {
            let d = d.trim_end_matches('.').to_ascii_lowercase();
            d != "localhost" && !d.ends_with(".localhost")
        }
        Some(url::Host::Ipv4(v4)) => is_public_ipv4(v4),
        Some(url::Host::Ipv6(v6)) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_public_ipv4(v4);
            }
            let seg0 = v6.segments()[0];
            let is_link_local = (seg0 & 0xffc0) == 0xfe80; // fe80::/10
            let is_unique_local = (seg0 & 0xfe00) == 0xfc00; // fc00::/7
            !(v6.is_loopback() || v6.is_unspecified() || is_link_local || is_unique_local)
        }
    }
}

#[cfg(feature = "string-advanced")]
pub(crate) fn is_valid_uri(s: &str) -> bool {
    url::Url::parse(s).is_ok()
}

pub(crate) fn is_valid_ipv4(s: &str) -> bool {
    s.parse::<Ipv4Addr>().is_ok()
}

pub(crate) fn is_valid_ipv6(s: &str) -> bool {
    s.parse::<Ipv6Addr>().is_ok()
}

pub(crate) fn is_valid_ip(s: &str) -> bool {
    is_valid_ipv4(s) || is_valid_ipv6(s)
}

pub(crate) fn is_valid_cidr(s: &str) -> bool {
    let Some((ip, prefix)) = s.split_once('/') else {
        return false;
    };
    // Digits only, no sign, no leading zeros.
    if prefix.is_empty()
        || prefix.len() > 3
        || !prefix.bytes().all(|b| b.is_ascii_digit())
        || (prefix.len() > 1 && prefix.starts_with('0'))
    {
        return false;
    }
    let Ok(prefix) = prefix.parse::<u8>() else {
        return false;
    };
    if is_valid_ipv4(ip) {
        prefix <= 32
    } else {
        is_valid_ipv6(ip) && prefix <= 128
    }
}

pub(crate) fn is_valid_mac(s: &str) -> bool {
    let parts: Vec<&str> = s.split(':').collect();
    parts.len() == 6
        && parts
            .iter()
            .all(|p| p.len() == 2 && p.bytes().all(|b| b.is_ascii_hexdigit()))
}

pub(crate) fn is_valid_hostname(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 253
        && s.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        })
}
