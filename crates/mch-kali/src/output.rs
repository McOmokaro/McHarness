//! Structured output parsing for Kali tool CLI results.
//!
//! Every wrapper emits a `ToolOutput` whose `parsed` field is a
//! `serde_json::Value` the agent can read directly.

use serde::{Deserialize, Serialize};

/// Normalized result of a single Kali tool invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutput {
    /// Tool binary name (e.g. "nmap")
    pub tool: String,
    /// Target the tool ran against
    pub target: String,
    /// Exit code from the process
    pub exit_code: i32,
    /// Wall-clock duration in milliseconds
    pub duration_ms: u64,
    /// Raw stdout (possibly truncated for very large dumps)
    pub raw: String,
    /// Raw stderr
    pub stderr: String,
    /// Structured, agent-readable parse of the output
    pub parsed: serde_json::Value,
    /// ISO-8601 timestamp of when the run completed
    pub timestamp: String,
}

impl ToolOutput {
    pub fn new(tool: impl Into<String>, target: impl Into<String>, exit_code: i32) -> Self {
        let tool = tool.into();
        let target = target.into();
        Self {
            tool,
            target,
            exit_code,
            duration_ms: 0,
            raw: String::new(),
            stderr: String::new(),
            parsed: serde_json::Value::Null,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Whether the tool reported success (exit code 0).
    pub fn ok(&self) -> bool {
        self.exit_code == 0
    }

    /// Attach a structured parse.
    pub fn with_parsed(mut self, parsed: serde_json::Value) -> Self {
        self.parsed = parsed;
        self
    }

    /// Attach the raw streams and duration.
    pub fn with_raw(mut self, stdout: String, stderr: String, duration_ms: u64) -> Self {
        self.raw = stdout;
        self.stderr = stderr;
        self.duration_ms = duration_ms;
        self
    }
}

/// Parse `subfinder` output: one host per line.
pub fn parse_subfinder(stdout: &str) -> serde_json::Value {
    let hosts: Vec<&str> = stdout
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    serde_json::json!({ "hosts": hosts, "count": hosts.len() })
}

/// Parse `ffuf -of json` output.
pub fn parse_ffuf(stdout: &str) -> serde_json::Value {
    match serde_json::from_str::<serde_json::Value>(stdout) {
        Ok(v) => v,
        Err(_) => serde_json::json!({ "raw": stdout, "note": "ffuf output not valid JSON" }),
    }
}

/// Parse `nuclei -jsonl` output: one JSON object per line.
pub fn parse_nuclei_jsonl(stdout: &str) -> serde_json::Value {
    let findings: Vec<serde_json::Value> = stdout
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    serde_json::json!({ "findings": findings, "count": findings.len() })
}

/// Parse `sqlmap` textual output into high-signal facts.
pub fn parse_sqlmap(stdout: &str) -> serde_json::Value {
    let mut injectable = false;
    let mut dbms: Option<String> = None;
    let mut techniques: Vec<String> = Vec::new();
    let mut payloads: Vec<String> = Vec::new();

    for line in stdout.lines() {
        let l = line.trim();
        if l.contains("is vulnerable") || l.contains("parameter") && l.contains("injectable") {
            injectable = true;
        }
        if let Some(rest) = l.strip_prefix("[INFO] the back-end DBMS is") {
            dbms = Some(rest.trim().to_string());
        }
        if let Some(rest) = l.strip_prefix("[INFO] testing") {
            techniques.push(rest.trim().to_string());
        }
        if l.starts_with("Payload:") {
            payloads.push(l.trim_start_matches("Payload:").trim().to_string());
        }
    }

    serde_json::json!({
        "injectable": injectable,
        "dbms": dbms,
        "techniques": techniques,
        "payloads": payloads,
    })
}

/// Parse `whatweb --log-json=-` output into a list of plugin matches.
pub fn parse_whatweb(stdout: &str) -> serde_json::Value {
    // whatweb --log-json emits a JSON array of objects.
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(stdout) {
        return v;
    }
    // Fallback: parse the human-readable line format.
    let mut plugins = Vec::new();
    for line in stdout.lines() {
        let l = line.trim();
        if l.is_empty() {
            continue;
        }
        // "http://host [200 OK] Country[...], WordPress[...], ..."
        if let Some(start) = l.find(']') {
            let rest = l[start + 1..].trim();
            for part in rest.split(',') {
                let p = part.trim();
                if let Some(bracket) = p.find('[') {
                    plugins.push(p[..bracket].trim().to_string());
                }
            }
        }
    }
    serde_json::json!({ "plugins": plugins })
}

/// Parse `nmap` gnmap/xml-lite output into open-port records.
pub fn parse_nmap(stdout: &str) -> serde_json::Value {
    let mut ports = Vec::new();
    let re = regex::Regex::new(r"^(\d+)/(tcp|udp)\s+(\w+)\s+(\S+)\s*(.*)$").unwrap();
    let mut in_port_section = false;
    for line in stdout.lines() {
        let l = line.trim();
        if l.starts_with("PORT") && l.contains("STATE") {
            in_port_section = true;
            continue;
        }
        if in_port_section {
            if l.is_empty() || l.starts_with("Nmap done") || l.starts_with("MAC ") {
                in_port_section = false;
                continue;
            }
            if let Some(caps) = re.captures(l) {
                ports.push(serde_json::json!({
                    "port": caps.get(1).unwrap().as_str().parse::<u16>().unwrap_or(0),
                    "protocol": caps.get(2).unwrap().as_str(),
                    "state": caps.get(3).unwrap().as_str(),
                    "service": caps.get(4).unwrap().as_str(),
                    "version": caps.get(5).map(|m| m.as_str()).unwrap_or(""),
                }));
            }
        }
    }
    serde_json::json!({ "ports": ports, "count": ports.len() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subfinder_parses_lines() {
        let out = "a.example.com\nb.example.com\n\n";
        let v = parse_subfinder(out);
        assert_eq!(v["count"], 2);
    }

    #[test]
    fn nmap_parses_open_ports() {
        let out = "PORT     STATE SERVICE VERSION\n80/tcp   open  http    nginx 1.18.0\n443/tcp  open  ssl/https\n\nNmap done";
        let v = parse_nmap(out);
        assert_eq!(v["count"], 2);
        assert_eq!(v["ports"][0]["port"], 80);
    }

    #[test]
    fn sqlmap_detects_injectable() {
        let out = "[INFO] testing connection to the target URL\n[INFO] GET parameter 'id' is vulnerable\n[INFO] the back-end DBMS is MySQL";
        let v = parse_sqlmap(out);
        assert_eq!(v["injectable"], true);
        assert_eq!(v["dbms"], "MySQL");
    }

    #[test]
    fn nuclei_parses_jsonl() {
        let out = "{\"template-id\":\"cve-2021-1234\"}\n{\"template-id\":\"exposed-panel\"}\n";
        let v = parse_nuclei_jsonl(out);
        assert_eq!(v["count"], 2);
    }
}
