//! Tool aggregation: many servers, one flat tool list.
//!
//! Forged from Goose `extension_manager/mod.rs::fetch_public_tools` +
//! `extension_manager/lease.rs::ToolCatalog::build` +
//! `reply_parts.rs::prepare_tools_for_provider`.

use std::collections::HashMap;

use serde_json::Value;

use crate::message::{Tool, TOOL_EXTENSION_META_KEY};

/// OpenAI rejects tool names outside `^[a-zA-Z0-9_-]{1,64}$`. Anything longer or
/// containing other characters is rewritten.
const MAX_PROVIDER_NAME: usize = 64;
const TRUNCATE_TO: usize = 55;

/// Rewrite a tool name so every provider accepts it, keeping collided or
/// truncated names distinct via a hash suffix.
///
/// Forged from Goose `lease.rs:180`. `db.query` and `db_query` — or two names
/// sharing their first 64 chars — stay different tools.
pub fn provider_safe_name(name: &str) -> String {
    let is_safe = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '-';
    if name.len() <= MAX_PROVIDER_NAME && name.chars().all(is_safe) {
        return name.to_string();
    }
    let sanitized: String = name
        .chars()
        .map(|c| if is_safe(c) { c } else { '_' })
        .take(TRUNCATE_TO)
        .collect();
    // Exactly 8 hex chars of the hash (a width spec is a minimum, not a cap).
    let hash = format!("{:016x}", fnv1a(name.as_bytes()));
    format!("{sanitized}_{}", &hash[..8])
}

/// FNV-1a — dependency-free, deterministic, good enough to disambiguate names.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Namespace a server-local tool for the model as `{owner}__{tool}` and stamp
/// the owner into `_meta`. First-class (unprefixed) extensions keep their name
/// but are still stamped.
pub fn namespace_tool(owner: &str, mut tool: Tool, unprefixed: bool) -> Tool {
    if !unprefixed {
        tool.name = format!("{owner}__{}", tool.name);
    }
    let mut meta = tool
        .meta
        .take()
        .and_then(|m| m.as_object().cloned())
        .unwrap_or_default();
    meta.insert(
        TOOL_EXTENSION_META_KEY.to_string(),
        Value::String(owner.to_string()),
    );
    tool.meta = Some(Value::Object(meta));
    tool
}

/// Repair a malformed input schema at the edge: ensure `type: object` and a
/// `properties` map. Returns whether anything changed.
pub fn normalize_schema(schema: &mut Value) -> bool {
    let mut changed = false;
    if let Some(obj) = schema.as_object_mut() {
        if !obj.contains_key("type") {
            obj.insert("type".into(), Value::String("object".into()));
            changed = true;
        }
        let is_object = obj.get("type") == Some(&Value::String("object".into()));
        if is_object && !obj.contains_key("properties") {
            obj.insert("properties".into(), Value::Object(Default::default()));
            changed = true;
        }
    }
    changed
}

/// The owner key stamped on a tool, if present.
pub fn tool_owner(tool: &Tool) -> Option<String> {
    tool.meta
        .as_ref()
        .and_then(|m| m.get(TOOL_EXTENSION_META_KEY))
        .and_then(Value::as_str)
        .map(str::to_string)
}

pub struct CatalogEntry {
    pub tool: Tool,
    /// The tool name as the owning server knows it (no `{owner}__` prefix).
    pub server_name: String,
    pub owner: String,
}

pub struct ToolCatalog {
    entries: Vec<CatalogEntry>,
    by_name: HashMap<String, usize>,
}

impl ToolCatalog {
    /// Merge already-namespaced tools from every source into one catalog.
    ///
    /// First-wins dedup on the provider-safe name; the result is sorted by name
    /// for stable ordering (which keeps a provider's prompt cache warm).
    pub fn build(sources: Vec<(String, Vec<Tool>)>) -> Self {
        let mut entries = Vec::new();
        let mut by_name: HashMap<String, usize> = HashMap::new();

        for (owner, tools) in sources {
            for tool in tools {
                let name = provider_safe_name(&tool.name);
                if by_name.contains_key(&name) {
                    tracing::warn!(
                        tool = %name,
                        extension = %owner,
                        "duplicate tool name, keeping the earlier extension's"
                    );
                    continue;
                }
                let server_name = tool
                    .name
                    .strip_prefix(&format!("{owner}__"))
                    .unwrap_or(&tool.name)
                    .to_string();
                let mut tool = tool;
                tool.name = name.clone();
                by_name.insert(name, entries.len());
                entries.push(CatalogEntry {
                    tool,
                    server_name,
                    owner: owner.clone(),
                });
            }
        }

        // Stable ordering for prompt-cache stability.
        entries.sort_by(|a, b| a.tool.name.cmp(&b.tool.name));
        let by_name = entries
            .iter()
            .enumerate()
            .map(|(i, e)| (e.tool.name.clone(), i))
            .collect();

        Self { entries, by_name }
    }

    pub fn get(&self, name: &str) -> Option<&CatalogEntry> {
        self.by_name.get(name).map(|&i| &self.entries[i])
    }

    pub fn entries(&self) -> &[CatalogEntry] {
        &self.entries
    }

    pub fn tools(&self) -> Vec<Tool> {
        self.entries.iter().map(|e| e.tool.clone()).collect()
    }

    pub fn tools_for(&self, owner: &str) -> Vec<Tool> {
        self.entries
            .iter()
            .filter(|e| e.owner == owner)
            .map(|e| e.tool.clone())
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Recover the real tool name from what a model emitted. Models rewrite the
    /// `__` separator into `.` (`dev__shell` -> `dev.shell`) or strip a
    /// `functions.` prefix. Forged from Goose `recover_mangled_tool_name`.
    pub fn recover_name(&self, emitted: &str) -> Option<String> {
        let trimmed = emitted.trim();
        let stripped = trimmed
            .strip_prefix("functions.")
            .or_else(|| trimmed.strip_prefix("functions:"))
            .unwrap_or(trimmed);

        for entry in &self.entries {
            let name = &entry.tool.name;
            if name == stripped {
                return Some(name.clone());
            }
            if let Some((ext, tool)) = name.split_once("__") {
                if format!("{ext}.{tool}") == stripped {
                    return Some(name.clone());
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tool(name: &str, schema: Value) -> Tool {
        Tool {
            name: name.to_string(),
            description: None,
            input_schema: schema,
            meta: None,
        }
    }

    #[test]
    fn safe_names_pass_through() {
        assert_eq!(provider_safe_name("shell"), "shell");
        assert_eq!(provider_safe_name("db_query"), "db_query");
        assert_eq!(provider_safe_name("a_b-c1"), "a_b-c1");
    }

    #[test]
    fn long_names_are_truncated_and_hashed() {
        let long = "x".repeat(90);
        let safe = provider_safe_name(&long);
        assert!(safe.len() <= MAX_PROVIDER_NAME, "len = {}", safe.len());
        assert!(safe.contains('_'));
    }

    #[test]
    fn dotted_and_underscored_names_stay_distinct() {
        let a = provider_safe_name("db.query");
        let b = provider_safe_name("db_query");
        assert_ne!(a, b);
    }

    #[test]
    fn namespaces_and_stamps_owner() {
        let t = namespace_tool("developer", tool("shell", json!({})), false);
        assert_eq!(t.name, "developer__shell");
        assert_eq!(tool_owner(&t).as_deref(), Some("developer"));
    }

    #[test]
    fn unprefixed_keeps_name_but_stamps() {
        let t = namespace_tool("core", tool("read_file", json!({})), true);
        assert_eq!(t.name, "read_file");
        assert_eq!(tool_owner(&t).as_deref(), Some("core"));
    }

    #[test]
    fn normalize_schema_fills_missing_fields() {
        let mut schema = json!({});
        assert!(normalize_schema(&mut schema));
        assert_eq!(schema["type"], json!("object"));
        assert!(schema["properties"].is_object());
        // Second call is a no-op.
        assert!(!normalize_schema(&mut schema));
    }

    #[test]
    fn dedup_keeps_earlier_extension() {
        let catalog = ToolCatalog::build(vec![
            (
                "alpha".into(),
                vec![namespace_tool("alpha", tool("search", json!({})), false)],
            ),
            (
                "beta".into(),
                vec![namespace_tool("beta", tool("search", json!({})), false)],
            ),
        ]);
        // Both namespaced names differ (alpha__search, beta__search); no clash.
        assert_eq!(catalog.entries().len(), 2);
    }

    #[test]
    fn dedup_on_identical_provider_name() {
        // Two sources both exposing an unprefixed "read_file".
        let catalog = ToolCatalog::build(vec![
            ("one".into(), vec![namespace_tool("one", tool("read_file", json!({})), true)]),
            ("two".into(), vec![namespace_tool("two", tool("read_file", json!({})), true)]),
        ]);
        assert_eq!(catalog.entries().len(), 1);
        assert_eq!(catalog.entries()[0].owner, "one");
    }

    #[test]
    fn tools_are_sorted_by_name() {
        let catalog = ToolCatalog::build(vec![(
            "dev".into(),
            vec![
                namespace_tool("dev", tool("zeta", json!({})), false),
                namespace_tool("dev", tool("alpha", json!({})), false),
            ],
        )]);
        let names: Vec<_> = catalog.entries().iter().map(|e| e.tool.name.as_str()).collect();
        assert_eq!(names, vec!["dev__alpha", "dev__zeta"]);
    }

    #[test]
    fn recovers_mangled_names() {
        let catalog = ToolCatalog::build(vec![(
            "developer".into(),
            vec![namespace_tool("developer", tool("shell", json!({})), false)],
        )]);
        assert_eq!(
            catalog.recover_name("developer.shell").as_deref(),
            Some("developer__shell")
        );
        assert_eq!(
            catalog.recover_name("functions.developer.shell").as_deref(),
            Some("developer__shell")
        );
        assert_eq!(
            catalog.recover_name("developer__shell").as_deref(),
            Some("developer__shell")
        );
        assert_eq!(catalog.recover_name("nope"), None);
    }

    #[test]
    fn server_name_is_stripped() {
        let catalog = ToolCatalog::build(vec![(
            "developer".into(),
            vec![namespace_tool("developer", tool("shell", json!({})), false)],
        )]);
        assert_eq!(catalog.entries()[0].server_name, "shell");
    }
}
