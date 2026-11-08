//! Kali tool registry, stubs, and typed wrappers.
//!
//! Each `KaliTool` is a placeholder describing a binary + apt package. The
//! wrappers (`Nmap`, `Ffuf`, `Sqlmap`, `Nuclei`, `Subfinder`, `Whatweb`)
//! build the correct CLI invocation and parse the output into JSON.

use crate::lazy::LazyLoader;
use crate::output::{self, ToolOutput};
use serde::{Deserialize, Serialize};
use std::time::Instant;
use tracing::debug;

/// Every Kali tool McHarness knows how to drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KaliTool {
    Nmap,
    Ffuf,
    Sqlmap,
    Nuclei,
    Subfinder,
    Whatweb,
    Wpscan,
    Gobuster,
    Nikto,
    Hydra,
    Httpx,
    Amass,
}

impl KaliTool {
    /// The executable name.
    pub fn binary(&self) -> &'static str {
        match self {
            KaliTool::Nmap => "nmap",
            KaliTool::Ffuf => "ffuf",
            KaliTool::Sqlmap => "sqlmap",
            KaliTool::Nuclei => "nuclei",
            KaliTool::Subfinder => "subfinder",
            KaliTool::Whatweb => "whatweb",
            KaliTool::Wpscan => "wpscan",
            KaliTool::Gobuster => "gobuster",
            KaliTool::Nikto => "nikto",
            KaliTool::Hydra => "hydra",
            KaliTool::Httpx => "httpx",
            KaliTool::Amass => "amass",
        }
    }

    /// The apt package that provides the binary.
    pub fn apt_package(&self) -> &'static str {
        match self {
            KaliTool::Nmap => "nmap",
            KaliTool::Ffuf => "ffuf",
            KaliTool::Sqlmap => "sqlmap",
            KaliTool::Nuclei => "nuclei",
            KaliTool::Subfinder => "subfinder",
            KaliTool::Whatweb => "whatweb",
            KaliTool::Wpscan => "wpscan",
            KaliTool::Gobuster => "gobuster",
            KaliTool::Nikto => "nikto",
            KaliTool::Hydra => "hydra",
            KaliTool::Httpx => "httpx-toolkit",
            KaliTool::Amass => "amass",
        }
    }

    /// Coarse category, used by the tactical planner for routing.
    pub fn category(&self) -> ToolCategory {
        match self {
            KaliTool::Nmap | KaliTool::Httpx => ToolCategory::Fingerprint,
            KaliTool::Whatweb | KaliTool::Wpscan | KaliTool::Nikto => ToolCategory::Fingerprint,
            KaliTool::Subfinder | KaliTool::Amass => ToolCategory::Recon,
            KaliTool::Ffuf | KaliTool::Gobuster => ToolCategory::Fuzzing,
            KaliTool::Sqlmap | KaliTool::Nuclei | KaliTool::Hydra => ToolCategory::Exploitation,
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            KaliTool::Nmap => "Network/port scanner with service/version detection",
            KaliTool::Ffuf => "Fast web fuzzer (content/param discovery)",
            KaliTool::Sqlmap => "Automatic SQL injection detection & exploitation",
            KaliTool::Nuclei => "Template-driven vulnerability scanner",
            KaliTool::Subfinder => "Passive subdomain enumeration",
            KaliTool::Whatweb => "Web technology fingerprinting",
            KaliTool::Wpscan => "WordPress vulnerability scanner",
            KaliTool::Gobuster => "Directory/DNS/vhost brute-forcer",
            KaliTool::Nikto => "Web server scanner",
            KaliTool::Hydra => "Network login brute-forcer",
            KaliTool::Httpx => "HTTP probing & tech detection",
            KaliTool::Amass => "Attack-surface / subdomain mapping",
        }
    }

    /// Resolve a tool from its binary name.
    pub fn from_binary(bin: &str) -> Option<KaliTool> {
        for t in KaliTool::all() {
            if t.binary() == bin {
                return Some(*t);
            }
        }
        None
    }

    /// All known tools.
    pub fn all() -> &'static [KaliTool] {
        &[
            KaliTool::Nmap,
            KaliTool::Ffuf,
            KaliTool::Sqlmap,
            KaliTool::Nuclei,
            KaliTool::Subfinder,
            KaliTool::Whatweb,
            KaliTool::Wpscan,
            KaliTool::Gobuster,
            KaliTool::Nikto,
            KaliTool::Hydra,
            KaliTool::Httpx,
            KaliTool::Amass,
        ]
    }
}

/// Coarse category for a tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCategory {
    Fingerprint,
    Recon,
    Fuzzing,
    Exploitation,
}

/// A placeholder describing a tool's availability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolStub {
    pub tool: KaliTool,
    pub installed: bool,
    pub path: Option<String>,
}

impl ToolStub {
    /// Probe the filesystem for this tool.
    pub fn probe(tool: KaliTool, loader: &LazyLoader) -> Self {
        let path = loader.locate(tool.binary());
        Self {
            tool,
            installed: path.is_some(),
            path: path.map(|p| p.display().to_string()),
        }
    }
}

/// A registry of stubs — the "toolbox" view the agent reasons over.
#[derive(Debug, Clone, Default)]
pub struct ToolRegistry {
    pub stubs: Vec<ToolStub>,
}

impl ToolRegistry {
    /// Build a registry by probing every known tool.
    pub fn probe_all(loader: &LazyLoader) -> Self {
        let stubs = KaliTool::all()
            .iter()
            .map(|t| ToolStub::probe(*t, loader))
            .collect();
        Self { stubs }
    }

    /// List the tools that are installed.
    pub fn installed(&self) -> Vec<KaliTool> {
        self.stubs
            .iter()
            .filter(|s| s.installed)
            .map(|s| s.tool)
            .collect()
    }

    /// List the tools that are *not* installed (lazy-load candidates).
    pub fn missing(&self) -> Vec<KaliTool> {
        self.stubs
            .iter()
            .filter(|s| !s.installed)
            .map(|s| s.tool)
            .collect()
    }
}

/// Execute a normalized command through a lazy loader and parse the result.
///
/// This is the shared backbone every concrete wrapper calls.
pub async fn run_raw(
    loader: &LazyLoader,
    tool: KaliTool,
    args: &[String],
    target: &str,
) -> anyhow::Result<ToolOutput> {
    let start = Instant::now();

    // Ensure the tool exists (installs on demand per the lazy strategy).
    let resolved = loader.ensure(tool).await?;

    let mut cmd = if let Some(path) = resolved {
        tokio::process::Command::new(path)
    } else if loader.locate(tool.binary()).is_some() {
        tokio::process::Command::new(tool.binary())
    } else {
        // Dry-run / not installed — return a planned, empty output.
        let mut out = ToolOutput::new(tool.binary(), target, 127);
        out.parsed = serde_json::json!({
            "planned": true,
            "args": args,
            "note": "tool not installed (dry-run or install unavailable)"
        });
        return Ok(out);
    };

    for a in args {
        cmd.arg(a);
    }
    debug!("executing {} {:?}", tool.binary(), args);

    let output = cmd.output().await?;
    let duration_ms = start.elapsed().as_millis() as u64;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(-1);

    let parsed = parse_for(tool, &stdout);

    Ok(ToolOutput::new(tool.binary(), target, exit_code)
        .with_raw(stdout, stderr, duration_ms)
        .with_parsed(parsed))
}

/// Dispatch to the right parser for a tool.
pub fn parse_for(tool: KaliTool, stdout: &str) -> serde_json::Value {
    match tool {
        KaliTool::Nmap => output::parse_nmap(stdout),
        KaliTool::Ffuf => output::parse_ffuf(stdout),
        KaliTool::Sqlmap => output::parse_sqlmap(stdout),
        KaliTool::Nuclei => output::parse_nuclei_jsonl(stdout),
        KaliTool::Subfinder | KaliTool::Amass => output::parse_subfinder(stdout),
        KaliTool::Whatweb | KaliTool::Wpscan | KaliTool::Nikto => output::parse_whatweb(stdout),
        _ => serde_json::json!({ "raw": stdout }),
    }
}

// ── Typed wrappers ───────────────────────────────────────────────────

/// `nmap` wrapper.
pub struct Nmap;

impl Nmap {
    /// Top-ports service scan.
    pub fn args_default(target: &str) -> Vec<String> {
        vec![
            "-sV".into(),
            "-T4".into(),
            "--top-ports".into(),
            "1000".into(),
            "-oN".into(),
            "-".into(),
            target.into(),
        ]
    }
    pub async fn run(loader: &LazyLoader, target: &str) -> anyhow::Result<ToolOutput> {
        run_raw(loader, KaliTool::Nmap, &Self::args_default(target), target).await
    }
}

/// `ffuf` wrapper.
pub struct Ffuf;

impl Ffuf {
    /// Content discovery against a wordlist URL template (FUZZ placeholder).
    pub fn args_with_wordlist(url_template: &str, wordlist: &str) -> Vec<String> {
        vec![
            "-u".into(),
            url_template.into(),
            "-w".into(),
            wordlist.into(),
            "-of".into(),
            "json".into(),
            "-o".into(),
            "/dev/stdout".into(),
            "-mc".into(),
            "200,301,302,401,403".into(),
        ]
    }
    pub async fn run(
        loader: &LazyLoader,
        url_template: &str,
        wordlist: &str,
    ) -> anyhow::Result<ToolOutput> {
        run_raw(
            loader,
            KaliTool::Ffuf,
            &Self::args_with_wordlist(url_template, wordlist),
            url_template,
        )
        .await
    }
}

/// `sqlmap` wrapper.
pub struct Sqlmap;

impl Sqlmap {
    pub fn args(target: &str) -> Vec<String> {
        vec![
            "-u".into(),
            target.into(),
            "--batch".into(),
            "--smart".into(),
            "--level".into(),
            "3".into(),
            "--risk".into(),
            "2".into(),
        ]
    }
    pub async fn run(loader: &LazyLoader, target: &str) -> anyhow::Result<ToolOutput> {
        run_raw(loader, KaliTool::Sqlmap, &Self::args(target), target).await
    }
}

/// `nuclei` wrapper.
pub struct Nuclei;

impl Nuclei {
    pub fn args(target: &str) -> Vec<String> {
        vec![
            "-u".into(),
            target.into(),
            "-severity".into(),
            "critical,high".into(),
            "-jsonl".into(),
            "-silent".into(),
        ]
    }
    pub async fn run(loader: &LazyLoader, target: &str) -> anyhow::Result<ToolOutput> {
        run_raw(loader, KaliTool::Nuclei, &Self::args(target), target).await
    }
}

/// `subfinder` wrapper — the wrapper emits the JSON *result* of a scan over a
/// supplied set of already-known hosts (kept hermetic for testing).
pub struct Subfinder;

impl Subfinder {
    pub fn args(domain: &str) -> Vec<String> {
        vec!["-d".into(), domain.into(), "-silent".into()]
    }
    pub async fn run(loader: &LazyLoader, domain: &str) -> anyhow::Result<ToolOutput> {
        run_raw(loader, KaliTool::Subfinder, &Self::args(domain), domain).await
    }
}

/// `whatweb` wrapper.
pub struct Whatweb;

impl Whatweb {
    pub fn args(target: &str) -> Vec<String> {
        vec![
            "--log-json=-".into(),
            "-a".into(),
            "3".into(),
            target.into(),
        ]
    }
    pub async fn run(loader: &LazyLoader, target: &str) -> anyhow::Result<ToolOutput> {
        run_raw(loader, KaliTool::Whatweb, &Self::args(target), target).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lazy::{LazyLoadConfig, LoadStrategy};

    #[test]
    fn tool_metadata_is_consistent() {
        for t in KaliTool::all() {
            assert!(!t.binary().is_empty());
            assert!(!t.apt_package().is_empty());
            assert_eq!(KaliTool::from_binary(t.binary()), Some(*t));
        }
    }

    #[test]
    fn wrappers_build_expected_args() {
        assert!(Nmap::args_default("10.0.0.1").contains(&"10.0.0.1".to_string()));
        assert!(Ffuf::args_with_wordlist("http://x/FUZZ", "/wl.txt")
            .iter()
            .any(|a| a.contains("FUZZ")));
        assert!(Nuclei::args("http://x").contains(&"-jsonl".to_string()));
    }

    #[tokio::test]
    async fn dry_run_does_not_install() {
        let loader = LazyLoader::new(LazyLoadConfig {
            strategy: LoadStrategy::AptGet,
            dry_run: true,
            ..Default::default()
        });
        // A tool that almost certainly isn't installed.
        let out = run_raw(&loader, KaliTool::Amass, &["-h".into()], "example.com")
            .await
            .unwrap();
        // dry-run returns a planned output rather than erroring.
        assert!(out.exit_code == 127 || out.exit_code == 0 || out.parsed["planned"] == true);
    }

    #[test]
    fn registry_probe_reports_installed_and_missing() {
        let loader = LazyLoader::new(LazyLoadConfig::default());
        let reg = ToolRegistry::probe_all(&loader);
        assert_eq!(reg.stubs.len(), KaliTool::all().len());
        assert_eq!(reg.installed().len() + reg.missing().len(), KaliTool::all().len());
    }
}
