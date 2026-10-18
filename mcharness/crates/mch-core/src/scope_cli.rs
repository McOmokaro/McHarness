//! CLI glue between the mch binary and mch-scope.

use mch_scope::{ScopeAuthority, ScopeVerdict};

pub fn check(host: &str) -> Result<ScopeVerdict, mch_scope::ScopeError> {
    let toml = std::env::var("MCH_SCOPE_TOML")
        .unwrap_or_else(|_| "[in_scope]\nhosts = []\n".to_string());
    let auth = ScopeAuthority::from_toml(&toml)?;
    auth.authorize(host)
}
