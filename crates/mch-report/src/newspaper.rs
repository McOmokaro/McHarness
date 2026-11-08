//! Markdown newspaper generation for Morning Report.
//!
//! Formats compiled data into a newspaper-style Markdown document
//! with sections, columns, and typographic flourishes.

use crate::compiler::ReportData;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Generates a Markdown newspaper from compiled report data.
#[derive(Debug, Default)]
pub struct NewspaperGenerator {
    /// Column count for layout (1-3)
    pub columns: usize,
    /// Include ASCII art header
    pub ascii_header: bool,
    /// Custom section order
    pub section_order: Vec<String>,
}

impl NewspaperGenerator {
    /// Create a new generator with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set column count (1-3).
    pub fn with_columns(mut self, cols: usize) -> Self {
        self.columns = cols.clamp(1, 3);
        self
    }

    /// Enable/disable ASCII art header.
    pub fn with_ascii_header(mut self, enabled: bool) -> Self {
        self.ascii_header = enabled;
        self
    }

    /// Set custom section order.
    pub fn with_section_order(mut self, order: Vec<String>) -> Self {
        self.section_order = order;
        self
    }

    /// Generate the full newspaper Markdown from report data.
    pub fn generate(&self, data: &ReportData) -> Result<String> {
        let mut out = String::new();

        if self.ascii_header {
            out.push_str(&self.render_header(&data.date));
            out.push('\n');
        }

        out.push_str(&self.render_masthead(&data.date));
        out.push_str("\n\n");

        let sections = self.ordered_sections(data);
        for (i, (name, content)) in sections.into_iter().enumerate() {
            if i > 0 {
                out.push_str("\n---\n\n");
            }
            out.push_str(&SectionFormatter::format_section(name, content, self.columns));
        }

        out.push_str("\n\n");
        out.push_str(&self.render_footer());

        Ok(out)
    }

    fn ordered_sections(&self, data: &ReportData) -> Vec<(String, String)> {
        let mut sections = Vec::new();

        // Default section order
        let default_order = vec![
            "headlines",
            "communications",
            "economic_engine",
            "hackathon_results",
            "metrics",
            "footnotes",
        ];

        let order = if self.section_order.is_empty() {
            default_order
        } else {
            self.section_order.clone()
        };

        for name in order {
            if let Some(content) = self.extract_section(data, &name) {
                sections.push((name.to_string(), content));
            }
        }

        sections
    }

    fn extract_section(&self, data: &ReportData, name: &str) -> Option<String> {
        match name {
            "headlines" => Some(self.format_headlines(&data.headlines)),
            "communications" => Some(self.format_communications(&data.communications)),
            "economic_engine" => Some(self.format_economic(&data.economic)),
            "hackathon_results" => Some(self.format_hackathon(&data.hackathon)),
            "metrics" => Some(self.format_metrics(&data.metrics)),
            "footnotes" => Some(self.format_footnotes(&data.footnotes)),
            _ => data.custom_sections.get(name).cloned(),
        }
    }

    fn format_headlines(&self, headlines: &[String]) -> String {
        if headlines.is_empty() {
            return "*No headlines today.*".to_string();
        }
        headlines
            .iter()
            .enumerate()
            .map(|(i, h)| format!("{}. {}", i + 1, h))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn format_communications(&self, comms: &[CommunicationItem]) -> String {
        if comms.is_empty() {
            return "*No communications logged.*".to_string();
        }
        comms
            .iter()
            .map(|c| format!("**{}** — {}", c.timestamp.format("%H:%M"), c.summary))
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    fn format_economic(&self, econ: &EconomicSummary) -> String {
        let mut lines = vec![
            format!("**Portfolio Value:** ${:.2}", econ.portfolio_value),
            format!("**Daily P&L:** ${:+.2} ({:+.2}%)", econ.daily_pnl, econ.daily_pnl_pct),
            format!("**Top Mover:** {} ({:+.2}%)", econ.top_mover, econ.top_mover_pct),
            format!("**Active Positions:** {}", econ.active_positions),
        ];
        if !econ.notes.is_empty() {
            lines.push(String::new());
            lines.push("**Notes:**".to_string());
            lines.extend(econ.notes.iter().map(|n| format!("- {}", n)));
        }
        lines.join("\n")
    }

    fn format_hackathon(&self, hack: &HackathonSummary) -> String {
        let mut lines = vec![
            format!("**Event:** {}", hack.event_name),
            format!("**Rank:** {} / {}", hack.rank, hack.total_teams),
            format!("**Score:** {:.1}", hack.score),
            format!("**Category:** {}", hack.category),
        ];
        if !hack.highlights.is_empty() {
            lines.push(String::new());
            lines.push("**Highlights:**".to_string());
            lines.extend(hack.highlights.iter().map(|h| format!("- {}", h)));
        }
        lines.join("\n")
    }

    fn format_metrics(&self, metrics: &HashMap<String, serde_json::Value>) -> String {
        if metrics.is_empty() {
            return "*No metrics recorded.*".to_string();
        }
        metrics
            .iter()
            .map(|(k, v)| format!("**{}:** {}", k, v))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn format_footnotes(&self, notes: &[String]) -> String {
        if notes.is_empty() {
            return String::new();
        }
        notes
            .iter()
            .enumerate()
            .map(|(i, n)| format!("{}. {}", i + 1, n))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn render_header(&self, date: &chrono::DateTime<chrono::Utc>) -> String {
        format!(
            r#"
╔═══════════════════════════════════════════════════════════════════╗
║                        MORNING REPORT                            ║
║                     {}                   ║
╚═══════════════════════════════════════════════════════════════════╝
"#,
            date.format("%A, %B %d, %Y")
        )
    }

    fn render_masthead(&self, date: &chrono::DateTime<chrono::Utc>) -> String {
        format!(
            "# Morning Report\n\n**{}** | Edition #{}",
            date.format("%B %d, %Y"),
            date.timestamp()
        )
    }

    fn render_footer(&self) -> String {
        "*Generated by Overnight Agent · mch-report*".to_string()
    }
}

/// Formats individual newspaper sections with column layout.
pub struct SectionFormatter;

impl SectionFormatter {
    /// Format a section with the given column count.
    pub fn format_section(title: &str, content: String, columns: usize) -> String {
        let mut out = String::new();
        out.push_str(&format!("## {}\n\n", title.to_uppercase()));

        if columns > 1 && content.lines().count() > 10 {
            out.push_str(&Self::columnize(&content, columns));
        } else {
            out.push_str(&content);
        }

        out
    }

    fn columnize(content: &str, columns: usize) -> String {
        let lines: Vec<&str> = content.lines().collect();
        let per_col = lines.len().div_ceil(columns);
        let mut cols = vec![Vec::new(); columns];

        for (i, line) in lines.into_iter().enumerate() {
            let col_idx = i / per_col;
            if col_idx < columns {
                cols[col_idx].push(line);
            }
        }

        let col_width = 80 / columns;
        let mut out = String::new();
        let max_rows = cols.iter().map(|c| c.len()).max().unwrap_or(0);

        for row in 0..max_rows {
            let mut line = String::new();
            for col in 0..columns {
                let cell = cols[col].get(row).unwrap_or(&"");
                line.push_str(&format!("{:<width$}", cell, width = col_width));
                if col < columns - 1 {
                    line.push_str(" | ");
                }
            }
            out.push_str(&line);
            out.push('\n');
        }

        out
    }
}

/// Individual communication log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunicationItem {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub channel: String,
    pub summary: String,
    pub priority: Priority,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Priority {
    Low,
    Normal,
    High,
    Critical,
}

/// Economic engine summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EconomicSummary {
    pub portfolio_value: f64,
    pub daily_pnl: f64,
    pub daily_pnl_pct: f64,
    pub top_mover: String,
    pub top_mover_pct: f64,
    pub active_positions: usize,
    pub notes: Vec<String>,
}

/// Hackathon results summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HackathonSummary {
    pub event_name: String,
    pub rank: usize,
    pub total_teams: usize,
    pub score: f64,
    pub category: String,
    pub highlights: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    fn sample_data() -> ReportData {
        ReportData {
            date: Utc.with_ymd_and_hms(2026, 1, 15, 6, 0, 0).unwrap(),
            headlines: vec!["Market opens strong", "New partnership announced".to_string()],
            communications: vec![CommunicationItem {
                timestamp: Utc.with_ymd_and_hms(2026, 1, 15, 5, 30, 0).unwrap(),
                channel: "slack".to_string(),
                summary: "Deploy completed successfully".to_string(),
                priority: Priority::Normal,
            }],
            economic: EconomicSummary {
                portfolio_value: 1_250_000.0,
                daily_pnl: 12_500.0,
                daily_pnl_pct: 1.01,
                top_mover: "AAPL".to_string(),
                top_mover_pct: 3.2,
                active_positions: 12,
                notes: vec!["Rebalancing scheduled for Friday".to_string()],
            },
            hackathon: HackathonSummary {
                event_name: "RustConf Hackathon".to_string(),
                rank: 3,
                total_teams: 47,
                score: 94.5,
                category: "Open Source".to_string(),
                highlights: vec!["Best CLI tool".to_string(), "Innovation award".to_string()],
            },
            metrics: HashMap::new(),
            footnotes: vec!["All times UTC".to_string()],
            custom_sections: HashMap::new(),
        }
    }

    #[test]
    fn generates_newspaper() {
        let gen = NewspaperGenerator::new()
            .with_ascii_header(true)
            .with_columns(1);
        let data = sample_data();
        let output = gen.generate(&data).unwrap();

        assert!(output.contains("MORNING REPORT"));
        assert!(output.contains("HEADLINES"));
        assert!(output.contains("COMMUNICATIONS"));
        assert!(output.contains("ECONOMIC ENGINE"));
        assert!(output.contains("HACKATHON RESULTS"));
        assert!(output.contains("METRICS"));
        assert!(output.contains("FOOTNOTES"));
    }

    #[test]
    fn section_formatter_single_column() {
        let formatted = SectionFormatter::format_section("TEST", "line1\nline2\nline3".to_string(), 1);
        assert!(formatted.contains("TEST"));
        assert!(formatted.contains("line1"));
    }

    #[test]
    fn section_formatter_multi_column() {
        let content = (1..20).map(|i| format!("Item {}", i)).collect::<Vec<_>>().join("\n");
        let formatted = SectionFormatter::format_section("TEST", content, 2);
        assert!(formatted.contains("TEST"));
        assert!(formatted.contains("Item 1"));
    }
}