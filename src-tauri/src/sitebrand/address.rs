//! Web addresses: what the user typed made into a full `https://` address,
//! links on a page made absolute, and local or private hosts refused.

use crate::error::{AppError, AppResult};

const MAX_ADDRESS: usize = 200;

/// The user's address as a full http(s) address. `theangaraa.com` becomes
/// `https://theangaraa.com/`.
pub fn normalise(input: &str) -> AppResult<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(AppError::InvalidInput("Type the website first, like theangaraa.com.".into()));
    }
    if trimmed.len() > MAX_ADDRESS || trimmed.chars().any(char::is_whitespace) {
        return Err(AppError::InvalidInput("That does not look like a website address.".into()));
    }
    let lower = trimmed.to_ascii_lowercase();
    let full = if lower.starts_with("https://") || lower.starts_with("http://") {
        trimmed.to_owned()
    } else if lower.contains("://") {
        return Err(AppError::InvalidInput("Only http and https websites can be read.".into()));
    } else {
        format!("https://{trimmed}")
    };
    check_public(&full)?;
    Ok(if authority_end(&full) == full.len() { format!("{full}/") } else { full })
}

/// Where the scheme ends (just after `://`).
fn scheme_end(url: &str) -> usize {
    url.find("://").map_or(0, |at| at + 3)
}

/// Where the host and port end.
fn authority_end(url: &str) -> usize {
    let start = scheme_end(url);
    url[start..].find(['/', '?', '#']).map_or(url.len(), |at| start + at)
}

/// The lowercase host of an absolute address.
pub fn host(url: &str) -> Option<String> {
    let authority = &url[scheme_end(url)..authority_end(url)];
    if authority.contains('@') || authority.starts_with('[') {
        return None;
    }
    let host = authority.split(':').next().unwrap_or("").trim_end_matches('.').to_ascii_lowercase();
    (!host.is_empty()).then_some(host)
}

/// Refuses addresses that point at this Mac or the local network, so a page
/// (or a redirect) cannot make the app read something private.
pub fn check_public(url: &str) -> AppResult<()> {
    let lower = url.to_ascii_lowercase();
    if !(lower.starts_with("https://") || lower.starts_with("http://")) {
        return Err(AppError::InvalidInput("Only http and https websites can be read.".into()));
    }
    let Some(host) = host(url) else {
        return Err(AppError::InvalidInput("That does not look like a website address.".into()));
    };
    let private_name = host == "localhost" || [".localhost", ".local", ".internal", ".lan", ".home"].iter().any(|end| host.ends_with(end));
    if private_name || !host.contains('.') || is_private_ip(&host) {
        return Err(AppError::InvalidInput("Only public websites can be read, not this Mac or the local network.".into()));
    }
    if !host.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.') {
        return Err(AppError::InvalidInput("That does not look like a website address.".into()));
    }
    Ok(())
}

fn is_private_ip(host: &str) -> bool {
    let Ok(ip) = host.parse::<std::net::Ipv4Addr>() else { return false };
    let [a, b, ..] = ip.octets();
    ip.is_loopback() || ip.is_private() || ip.is_link_local() || ip.is_unspecified() || ip.is_broadcast() || (a == 100 && (64..128).contains(&b))
}

/// A link on the page at `base`, made absolute. `None` for links that are
/// not web addresses (`data:`, `javascript:`, `mailto:` and the like).
pub fn resolve(base: &str, href: &str) -> Option<String> {
    let href = href.trim();
    let href = href.split('#').next().unwrap_or("");
    if href.is_empty() {
        return None;
    }
    let lower = href.to_ascii_lowercase();
    if lower.starts_with("https://") || lower.starts_with("http://") {
        return Some(href.to_owned());
    }
    if let Some(colon) = href.find(':') {
        if !href[..colon].contains(['/', '?']) {
            return None;
        }
    }
    let scheme = &base[..scheme_end(base)];
    let origin = &base[..authority_end(base)];
    if let Some(rest) = href.strip_prefix("//") {
        return Some(format!("{scheme}{rest}"));
    }
    if href.starts_with('/') {
        return Some(format!("{origin}{href}"));
    }
    let path = &base[authority_end(base)..];
    let path = path.split(['?', '#']).next().unwrap_or("");
    let folder = path.rfind('/').map_or("/", |at| &path[..=at]);
    Some(format!("{origin}{folder}{href}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_addresses_become_full_ones() {
        assert_eq!(normalise(" theangaraa.com ").unwrap(), "https://theangaraa.com/");
        assert_eq!(normalise("http://example.com/shop?x=1").unwrap(), "http://example.com/shop?x=1");
        assert_eq!(normalise("").unwrap_err().code(), "invalid_input");
        assert_eq!(normalise("ftp://example.com").unwrap_err().code(), "invalid_input");
        assert_eq!(normalise("file:///etc/passwd").unwrap_err().code(), "invalid_input");
        assert_eq!(normalise("my site.com").unwrap_err().code(), "invalid_input");
    }

    #[test]
    fn local_and_private_hosts_are_refused() {
        for url in ["localhost", "http://127.0.0.1", "192.168.1.4", "10.0.0.2:8080", "printer.local", "http://[::1]/", "user@example.com", "intranet", "172.16.0.1", "100.64.1.1", "169.254.169.254"] {
            assert!(normalise(url).is_err(), "{url}");
        }
        assert!(normalise("8.8.8.8").is_ok());
        assert!(check_public("https://example.com/").is_ok());
    }

    #[test]
    fn links_are_made_absolute() {
        let base = "https://shop.example.com/en/about.html?q=1";
        assert_eq!(resolve(base, "/logo.svg").unwrap(), "https://shop.example.com/logo.svg");
        assert_eq!(resolve(base, "img/logo.png").unwrap(), "https://shop.example.com/en/img/logo.png");
        assert_eq!(resolve(base, "//cdn.example.net/a.css").unwrap(), "https://cdn.example.net/a.css");
        assert_eq!(resolve(base, "https://other.com/x.css#top").unwrap(), "https://other.com/x.css");
        assert_eq!(resolve("https://example.com", "a.css").unwrap(), "https://example.com/a.css");
        assert!(resolve(base, "data:image/png;base64,AAA").is_none());
        assert!(resolve(base, "javascript:alert(1)").is_none());
        assert!(resolve(base, "#main").is_none());
    }
}
