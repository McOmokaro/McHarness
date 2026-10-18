//! mch-scope — fail-closed scope authority.
//!
//! Mined from H-mmer/pentest-agents (`tools/scope_check.py`, `tools/scope_hook.py`):
//! three-state verdicts, deny-wins matching, placeholder-scope refusal, and a
//! pre-execution interceptor that runs on ALL tool payloads (the source only
//! hooked Bash commands).

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;
use std::net::IpAddr;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ScopeError {
    #[error("no in-scope targets defined — scope authority is empty (fail-closed)")]
    NoScopeFile,
    #[error("failed to parse scope definition: {0}")]
    Parse(String),
    #[error("execution denied: {0}")]
    Denied(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScopeVerdict {
    InScope,
    OutOfScope,
    Uncertain,
}

impl fmt::Display for ScopeVerdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScopeVerdict::InScope => write!(f, "IN_SCOPE"),
            ScopeVerdict::OutOfScope => write!(f, "OUT_OF_SCOPE"),
            ScopeVerdict::Uncertain => write!(f, "UNCERTAIN"),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScopeSet {
    #[serde(default)]
    pub in_scope_hosts: Vec<String>,
    #[serde(default)]
    pub in_scope_cidrs: Vec<String>,
    #[serde(default)]
    pub out_of_scope_hosts: Vec<String>,
    #[serde(default)]
    pub out_of_scope_cidrs: Vec<String>,
}

impl ScopeSet {
    /// Minimal TOML-section parser: `[section]` headers plus `key = [ ... ]` lists.
    pub fn from_toml_str(s: &str) -> Result<Self, ScopeError> {
        let mut set = ScopeSet::default();
        let mut section = "";
        for line in s.lines() {
            let line = strip_comment(line).trim();
            if line.is_empty() {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                section = &line[1..line.len() - 1];
                continue;
            }
            let Some((key, val)) = line.split_once('=') else {
                continue;
            };
            let items = parse_string_list(val);
            match (section, key.trim()) {
                ("in_scope", "hosts") => set.in_scope_hosts = items,
                ("in_scope", "cidrs") => set.in_scope_cidrs = items,
                ("out_of_scope", "hosts") => set.out_of_scope_hosts = items,
                ("out_of_scope", "cidrs") => set.out_of_scope_cidrs = items,
                _ => {}
            }
        }
        Ok(set)
    }
}

fn strip_comment(line: &str) -> &str {
    match line.find('#') {
        Some(i) => &line[..i],
        None => line,
    }
}

fn parse_string_list(val: &str) -> Vec<String> {
    let inner = val.trim().trim_start_matches('[').trim_end_matches(']');
    inner
        .split(',')
        .map(|s| s.trim().trim_matches(|c| c == '"' || c == '\'').to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// The authority: only explicit `InScope` verdicts allow execution. An empty
/// scope is refused at the gate — the "empty scope = everything allowed" hole
/// from the source lineage is closed structurally.
pub struct ScopeAuthority {
    set: ScopeSet,
    in_cidrs: Vec<(IpAddr, u8)>,
    out_cidrs: Vec<(IpAddr, u8)>,
}

impl ScopeAuthority {
    pub fn new(set: ScopeSet) -> Result<Self, ScopeError> {
        let in_cidrs = set
            .in_scope_cidrs
            .iter()
            .map(|c| parse_cidr(c))
            .collect::<Result<Vec<_>, _>>()?;
        let out_cidrs = set
            .out_of_scope_cidrs
            .iter()
            .map(|c| parse_cidr(c))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { set, in_cidrs, out_cidrs })
    }

    pub fn from_toml(s: &str) -> Result<Self, ScopeError> {
        Self::new(ScopeSet::from_toml_str(s)?)
    }

    pub fn is_empty(&self) -> bool {
        self.set.in_scope_hosts.is_empty() && self.in_cidrs.is_empty()
    }

    /// Deny-wins evaluation: out_of_scope checked first.
    pub fn check_host(&self, host: &str) -> ScopeVerdict {
        let host = normalize_host(host);
        if self.set.out_of_scope_hosts.iter().any(|h| matches_host(&normalize_host(h), &host))
            || self.out_cidrs.iter().any(|(net, prefix)| ip_in_cidr(&host, *net, *prefix))
        {
            return ScopeVerdict::OutOfScope;
        }
        if self.set.in_scope_hosts.iter().any(|h| matches_host(&normalize_host(h), &host))
            || self.in_cidrs.iter().any(|(net, prefix)| ip_in_cidr(&host, *net, *prefix))
        {
            return ScopeVerdict::InScope;
        }
        ScopeVerdict::Uncertain
    }

    /// The gate. Only `InScope` proceeds; `Uncertain` and `OutOfScope` are
    /// fail-closed denials.
    pub fn authorize(&self, host: &str) -> Result<ScopeVerdict, ScopeError> {
        if self.is_empty() {
            return Err(ScopeError::NoScopeFile);
        }
        match self.check_host(host) {
            SH: Some(ScopeVerdict::InScope) => Ok(ScopeVerdict::InScope),
            v => Err(ScopeError::Denied(format!("host `{host}` verdict {v}"))),
        }
    }

    /// Pre-execution interceptor: extract candidate hosts from any tool-call
    /// payload and deny if any is out of scope.
    pub fn intercept(&self, payload: &str) -> Result<Vec<String>, ScopeError> {
        let mut offenders = Vec::new();
        for host in extract_hosts(payload) {
            if self.authorize(&host).is_err() {
                offenders.push(host);
            }
        }
        if offenders.is_empty() {
            Ok(Vec::new())
        } else {
            Err(ScopeError::Denied(format!(
                "out-of-scope hosts referenced: {}",
                offenders.join(", ")
            )))
        }
    }
}

fn normalize_host(h: &str) -> String {
    h.trim()
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .split(['/', ':', '?'])
        .next()
        .unwrap_or("")
        .trim_end_matches('.')
        .to_ascii_lowercase()
}

/// `*.example.com` matches subdomains AND the base itself.
fn matches_host(pattern: &str, host: &str) -> bool {
    if pattern == host {
        return true;
    }
    if let Some(base) = pattern.strip_prefix("*.") {
        return host == base || host.ends_with(&format!(".{base}"));
    }
    false
}

fn parse_cidr(s: &str) -> Result<(IpAddr, u8), ScopeError> {
    let (addr, prefix) = s
        .split_once('/')
        .ok_or_else(|| ScopeError::Parse(format!("cidr missing prefix: {s}")))?;
    let addr: IpAddr = addr
        .trim()
        .parse()
        .map_err(|_| ScopeError::Parse(format!("bad cidr address: {s}")))?;
    let max = match addr { IpAddr::V4(_) => 32u8, IpAddr::V6(_) => 128u8 };
    let prefix: u8 = prefix
        .trim()
        .parse()
        .map_err(|_| ScopeError::Parse(format!("cidr prefix out of range: {s}")))?;
    if prefix > max {
        return Err(ScopeError::Parse(format!("cidr prefix out of range: {s}")));
    }
    Ok((addr, prefix))
}

fn ip_in_cidr(host: &str, net: IpAddr, prefix: u8) -> bool {
    let Ok(ip) = host.parse::<IpAddr>() else { return false };
    match (ip, net) {
        (IpAddr::V4(a), IpAddr::V4(b)) => {
            let shift = 32 - prefix.min(32) as u32;
            u32::from(a) >> shift == u32::from(b) >> shift
        }
        (IpAddr::V6(a), IpAddr::V6(b)) => {
            let shift = 128 - prefix.min(128) as u32;
            u128::from(a) >> shift == u128::from(b) >> shift
        }
        _ => false,
    }
}

/// Host extraction from arbitrary payloads: scheme-ful URLs and bare
/// dot-delimited hostnames both count.
pub fn extract_hosts(payload: &str) -> HashSet<String> {
    let mut out = HashSet::new();
    for token in payload.split(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == '=' || c == ',') {
        for seg in token.split(|c: char| !c.is_ascii_alphanumeric() && c != '.' && c != '-' && c != ':') {
            if is_hostish(seg) {
                out.insert(seg.to_string());
            }
        }
    }
    out
}

fn is_hostish(s: &str) -> bool {
    if s.len() < 4 || !s.contains('.') || s.contains(':') {
        return false;
    }
    let parts: Vec<&str> = s.split('.').collect();
    if parts.iter().any(|p| p.is_empty() || !p.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')) {
        return false;
    }
    match parts.last() {
        Some(tld) => tld.len() >= 2 && tld.chars().all(|c| c.is_ascii_alphabetic()),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCOPE: &str = r#"
[in_scope]
hosts = ["*.example.com"]
cidrs = ["10.0.0.0/8", "192.168.1.0/24"]

[out_of_scope]
hosts = ["mail.example.com"]
cidrs = []
"#;

    fn auth() -> ScopeAuthority {
        ScopeAuthority::from_toml(SCOPE).unwrap()
    }

    #[test]
    fn in_scope_wildcard_and_base() {
        assert_eq!(auth().check_host("sub.example.com"), ScopeVerdict::InScope);
        assert_eq!(auth().check_host("example.com"), ScopeVerdict::InScope);
    }

    #[test]
    fn deny_wins() {
        assert_eq!(auth().check_host("mail.example.com"), ScopeVerdict::OutOfScope);
    }

    #[test]
    fn uncertain_is_fail_closed() {
        assert_eq!(auth().check_host("random.org"), ScopeVerdict::Uncertain);
        assert!(auth().authorize("random.org").is_err());
    }

    #[test]
    fn empty_scope_never_allows() {
        let a = ScopeAuthority::new(ScopeSet::default()).unwrap();
        assert!(a.authorize("example.com").is_err());
    }

    #[test]
    fn cidr_matching() {
        assert_eq!(auth().check_host("10.1.2.3"), ScopeVerdict::InScope);
        assert_eq!(auth().check_host("192.168.1.77"), ScopeVerdict::InScope);
        assert_eq!(auth().check_host("11.0.0.1"), ScopeVerdict::Uncertain);
    }

    #[test]
    fn interceptor_blocks_urls_in_payload() {
        let err = auth()
            .intercept("curl https://random.org/x --data evil")
            .unwrap_err();
        assert!(err.to_string().contains("random.org"));
        assert!(auth().intercept("curl https://example.com/x").is_ok());
    }
}
