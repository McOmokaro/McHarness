//! Environment hardening.
//!
//! Forged from Goose `crates/goose/src/agents/extension.rs:91`. An MCP server's
//! config may carry an `env` block. Without this filter, that block can set a
//! dynamic-loader preload or an interpreter hook — i.e. a config becomes RCE.
//! These 31 keys are refused at construction.

use std::collections::HashMap;

use tracing::warn;

pub const DISALLOWED_KEYS: [&str; 31] = [
    // Binary / executable path manipulation
    "PATH",
    "PATHEXT",
    "SystemRoot",
    "windir",
    // Linux dynamic-linker hijacking
    "LD_LIBRARY_PATH",
    "LD_PRELOAD",
    "LD_AUDIT",
    "LD_DEBUG",
    "LD_BIND_NOW",
    "LD_ASSUME_KERNEL",
    // macOS dynamic-linker hijacking
    "DYLD_LIBRARY_PATH",
    "DYLD_INSERT_LIBRARIES",
    "DYLD_FRAMEWORK_PATH",
    // Interpreter / runtime hook injection
    "PYTHONPATH",
    "PYTHONHOME",
    "NODE_OPTIONS",
    "RUBYOPT",
    "GEM_PATH",
    "GEM_HOME",
    "CLASSPATH",
    "GO111MODULE",
    "GOROOT",
    // Windows process / DLL hijacking and path redirection
    "APPINIT_DLLS",
    "SESSIONNAME",
    "ComSpec",
    "TEMP",
    "TMP",
    "LOCALAPPDATA",
    "USERPROFILE",
    "HOMEDRIVE",
    "HOMEPATH",
];

pub fn is_disallowed(key: &str) -> bool {
    DISALLOWED_KEYS
        .iter()
        .any(|disallowed| disallowed.eq_ignore_ascii_case(key))
}

/// Filter a raw env map, dropping disallowed keys with a warning.
pub fn sanitize_env(raw: HashMap<String, String>) -> HashMap<String, String> {
    let mut validated = HashMap::new();
    for (key, value) in raw {
        if is_disallowed(&key) {
            warn!(key = %key, "skipping disallowed env var");
            continue;
        }
        validated.insert(key, value);
    }
    validated
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_loader_and_path_hijacks() {
        let mut raw = HashMap::new();
        raw.insert("PATH".into(), "/evil".into());
        raw.insert("LD_PRELOAD".into(), "/evil.so".into());
        raw.insert("NODE_OPTIONS".into(), "--require /evil".into());
        raw.insert("FOO".into(), "bar".into());
        let out = sanitize_env(raw);
        assert_eq!(out.len(), 1);
        assert_eq!(out.get("FOO").map(String::as_str), Some("bar"));
    }

    #[test]
    fn case_insensitive_match() {
        assert!(is_disallowed("path"));
        assert!(is_disallowed("ld_preload"));
        assert!(is_disallowed("ComSpec"));
        assert!(!is_disallowed("DATABASE_URL"));
    }
}
