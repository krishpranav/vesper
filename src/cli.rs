use crate::export::{self, ExportFormat};
use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(
    name = "vesper",
    version,
    about = "Professional OSINT Username Scanner",
    long_about = "vesper - User OSINT Across Social Networks.\n\nA powerful tool for investigating usernames across 2000+ social networks and websites."
)]
pub struct Cli {
    #[arg(required_unless_present_any = ["test", "download", "input"])]
    pub usernames: Vec<String>,

    #[arg(long = "no-color")]
    pub no_color: bool,

    #[arg(long = "update")]
    pub update: bool,

    #[arg(short = 't', long = "tor")]
    pub tor: bool,

    #[arg(short = 's', long = "screenshot")]
    pub screenshot: bool,

    #[arg(short = 'v', long = "verbose")]
    pub verbose: bool,

    #[arg(short = 'd', long = "download")]
    pub download: bool,

    #[arg(long = "database", value_name = "DATABASE")]
    pub database: Option<String>,

    #[arg(long = "site", value_name = "SITE")]
    pub site: Option<String>,

    #[arg(short = 'o', long = "output", value_name = "FILE")]
    pub output: Option<String>,

    /// Export format for --output: json, csv, or txt (auto-detected from extension if omitted)
    #[arg(long = "format", value_name = "FORMAT", value_parser = ["json", "csv", "txt"])]
    pub format: Option<String>,

    /// Request timeout in seconds
    #[arg(long = "timeout", value_name = "SECONDS", default_value_t = 10)]
    pub timeout: u64,

    /// Read usernames from file (one per line)
    #[arg(short = 'i', long = "input", value_name = "FILE")]
    pub input: Option<String>,

    #[arg(long = "test")]
    pub test: bool,
}

impl Cli {
    pub fn parse_args() -> Self {
        Cli::parse()
    }

    pub fn max_workers(&self) -> usize {
        if self.screenshot {
            8
        } else {
            32
        }
    }

    pub fn database_path(&self) -> String {
        self.database
            .clone()
            .unwrap_or_else(|| "data.json".to_string())
    }

    /// Resolve the export format from --format flag, output file extension, or default to JSON.
    pub fn output_format(&self) -> ExportFormat {
        export::resolve_format(
            self.format.as_deref(),
            self.output.as_deref().unwrap_or(""),
        )
    }
}
