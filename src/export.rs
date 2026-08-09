use crate::core::ResultStatus;
use anyhow::{Context, Result};
use std::fmt::Write as FmtWrite;
use std::fs;
use std::path::Path;

use crate::{ScanReport, UsernameScanReport};

/// Supported export formats for scan reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Json,
    Csv,
    Txt,
    Markdown,
}

impl ExportFormat {
    /// Detect format from a file extension string.
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "json" => Some(ExportFormat::Json),
            "csv" => Some(ExportFormat::Csv),
            "txt" | "text" => Some(ExportFormat::Txt),
            "md" | "markdown" => Some(ExportFormat::Markdown),
            _ => None,
        }
    }

    /// Detect format from a file path's extension.
    pub fn from_path(path: &str) -> Option<Self> {
        Path::new(path)
            .extension()
            .and_then(|ext| ext.to_str())
            .and_then(Self::from_extension)
    }

    /// Parse a format name string (e.g. from --format flag).
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "json" => Some(ExportFormat::Json),
            "csv" => Some(ExportFormat::Csv),
            "txt" | "text" => Some(ExportFormat::Txt),
            "md" | "markdown" => Some(ExportFormat::Markdown),
            _ => None,
        }
    }
}

/// Resolve the output format from an explicit --format flag, the file extension, or default to JSON.
pub fn resolve_format(format_flag: Option<&str>, output_path: &str) -> ExportFormat {
    // Explicit --format flag takes priority
    if let Some(name) = format_flag {
        if let Some(fmt) = ExportFormat::from_name(name) {
            return fmt;
        }
    }

    // Fall back to file extension detection
    if let Some(fmt) = ExportFormat::from_path(output_path) {
        return fmt;
    }

    // Default to JSON
    ExportFormat::Json
}

/// Write a scan report to disk in the specified format.
pub fn write_report(
    output_path: &str,
    database_sites: usize,
    usernames: Vec<UsernameScanReport>,
    format: ExportFormat,
) -> Result<()> {
    let report = ScanReport {
        generated_at: chrono::Utc::now().to_rfc3339(),
        database_sites,
        usernames,
    };

    // Ensure parent directory exists
    if let Some(parent) = Path::new(output_path).parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create report directory: {:?}", parent))?;
        }
    }

    let content = match format {
        ExportFormat::Json => render_json(&report)?,
        ExportFormat::Csv => render_csv(&report),
        ExportFormat::Txt => render_txt(&report),
        ExportFormat::Markdown => render_markdown(&report),
    };

    fs::write(output_path, content)
        .with_context(|| format!("Failed to write scan report: {}", output_path))?;

    let format_label = match format {
        ExportFormat::Json => "JSON",
        ExportFormat::Csv => "CSV",
        ExportFormat::Txt => "Text",
        ExportFormat::Markdown => "Markdown",
    };

    println!("[✓] {} report written to {}", format_label, output_path);

    Ok(())
}

// ── JSON ──────────────────────────────────────────────────────────────────────

fn render_json(report: &ScanReport) -> Result<String> {
    serde_json::to_string_pretty(report).context("Failed to serialize report to JSON")
}

// ── CSV ───────────────────────────────────────────────────────────────────────

fn render_csv(report: &ScanReport) -> String {
    let mut buf = String::new();

    // Header row
    writeln!(buf, "username,site,url,status,confidence,found").unwrap();

    for scan in &report.usernames {
        for result in &scan.results {
            writeln!(
                buf,
                "{},{},{},{},{:.2},{}",
                csv_escape(&result.username),
                csv_escape(&result.site),
                csv_escape(&result.link),
                csv_escape(result.status.as_tag()),
                result.confidence,
                result.exist,
            )
            .unwrap();
        }
    }

    buf
}

/// Escape a field for CSV: wrap in double-quotes if it contains comma, quote, or newline.
fn csv_escape(field: &str) -> String {
    if field.contains(',') || field.contains('"') || field.contains('\n') {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

// ── Text ──────────────────────────────────────────────────────────────────────

fn render_txt(report: &ScanReport) -> String {
    let mut buf = String::new();

    writeln!(buf, "═══════════════════════════════════════════════════════════════").unwrap();
    writeln!(buf, "  VESPER — OSINT Username Scan Report").unwrap();
    writeln!(buf, "═══════════════════════════════════════════════════════════════").unwrap();
    writeln!(buf, "  Generated : {}", report.generated_at).unwrap();
    writeln!(buf, "  Database  : {} sites", report.database_sites).unwrap();
    writeln!(buf, "═══════════════════════════════════════════════════════════════").unwrap();

    for scan in &report.usernames {
        writeln!(buf).unwrap();
        writeln!(buf, "───────────────────────────────────────────────────────────────").unwrap();
        writeln!(buf, "  Username : {}", scan.username).unwrap();
        writeln!(buf, "  Checked  : {}", scan.checked).unwrap();
        writeln!(buf, "  Found    : {}", scan.found).unwrap();
        writeln!(buf, "  Confirmed: {}   Likely: {}   Blocked: {}", scan.confirmed, scan.likely, scan.blocked).unwrap();
        writeln!(buf, "  Time     : {:.2}s", scan.elapsed_secs).unwrap();
        writeln!(buf, "───────────────────────────────────────────────────────────────").unwrap();

        // Group results: found first, then not-found
        let mut found_results: Vec<_> = scan.results.iter().filter(|r| r.exist).collect();
        let mut not_found_results: Vec<_> = scan.results.iter().filter(|r| !r.exist).collect();

        // Sort alphabetically by site within each group
        found_results.sort_by(|a, b| a.site.to_lowercase().cmp(&b.site.to_lowercase()));
        not_found_results.sort_by(|a, b| a.site.to_lowercase().cmp(&b.site.to_lowercase()));

        if !found_results.is_empty() {
            writeln!(buf).unwrap();
            writeln!(buf, "  [+] FOUND PROFILES:").unwrap();
            writeln!(buf).unwrap();
            for result in &found_results {
                let confidence_str = if result.confidence > 0.0 {
                    format!(" ({:.0}%)", result.confidence * 100.0)
                } else {
                    String::new()
                };
                let status_label = match result.status {
                    ResultStatus::Confirmed => "CONFIRMED",
                    ResultStatus::Likely => "LIKELY",
                    ResultStatus::Private => "PRIVATE",
                    _ => "",
                };
                writeln!(
                    buf,
                    "      [+] {:<25} {}  [{}{}]",
                    result.site, result.link, status_label, confidence_str
                )
                .unwrap();
            }
        }

        if !not_found_results.is_empty() {
            let error_results: Vec<_> = not_found_results
                .iter()
                .filter(|r| r.error)
                .collect();
            let blocked_results: Vec<_> = not_found_results
                .iter()
                .filter(|r| r.status == ResultStatus::Blocked)
                .collect();

            if !error_results.is_empty() {
                writeln!(buf).unwrap();
                writeln!(buf, "  [!] ERRORS ({}):", error_results.len()).unwrap();
                writeln!(buf).unwrap();
                for result in &error_results {
                    writeln!(
                        buf,
                        "      [!] {:<25} {}",
                        result.site, result.error_msg
                    )
                    .unwrap();
                }
            }

            if !blocked_results.is_empty() {
                writeln!(buf).unwrap();
                writeln!(buf, "  [⊗] BLOCKED ({}):", blocked_results.len()).unwrap();
                writeln!(buf).unwrap();
                for result in &blocked_results {
                    writeln!(
                        buf,
                        "      [⊗] {:<25} {}",
                        result.site, result.error_msg
                    )
                    .unwrap();
                }
            }
        }
    }

    writeln!(buf).unwrap();
    writeln!(buf, "═══════════════════════════════════════════════════════════════").unwrap();
    writeln!(buf, "  End of Report").unwrap();
    writeln!(buf, "═══════════════════════════════════════════════════════════════").unwrap();

    buf
}

// ── Markdown ──────────────────────────────────────────────────────────────────

fn render_markdown(report: &ScanReport) -> String {
    let mut buf = String::new();
    
    writeln!(buf, "# Vesper Scan Report\n").unwrap();
    writeln!(buf, "- **Generated:** {}", report.generated_at).unwrap();
    writeln!(buf, "- **Database Sites:** {}\n", report.database_sites).unwrap();
    
    for scan in &report.usernames {
        writeln!(buf, "## Username: `{}`\n", scan.username).unwrap();
        writeln!(buf, "- **Checked:** {}", scan.checked).unwrap();
        writeln!(buf, "- **Found:** {}", scan.found).unwrap();
        writeln!(buf, "- **Time:** {:.2}s\n", scan.elapsed_secs).unwrap();
        
        let found_results: Vec<_> = scan.results.iter().filter(|r| r.exist).collect();
        if !found_results.is_empty() {
            writeln!(buf, "### Found Profiles\n").unwrap();
            writeln!(buf, "| Site | Link | Status | Confidence |").unwrap();
            writeln!(buf, "|------|------|--------|------------|").unwrap();
            for r in &found_results {
                let conf = if r.confidence > 0.0 { format!("{:.0}%", r.confidence * 100.0) } else { "-".to_string() };
                writeln!(buf, "| {} | {} | {} | {} |", r.site, r.link, r.status.as_tag(), conf).unwrap();
            }
            writeln!(buf).unwrap();
        }
    }
    
    buf
}
