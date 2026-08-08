use crate::scraper::ScraperStats;
use colored::*;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::sync::Arc;

#[derive(Clone)]
pub struct Logger {
    pub no_color: bool,
    pub verbose: bool,
    #[allow(dead_code)]
    multi_progress: Arc<MultiProgress>,
}

impl Logger {
    pub fn new(no_color: bool, verbose: bool) -> Self {
        Self {
            no_color,
            verbose,
            multi_progress: Arc::new(MultiProgress::new()),
        }
    }

    pub fn print_banner(&self, username: &str) {
        if self.no_color {
            println!("\nInvestigating {} on:", username);
        } else {
            println!("\n🔎 Investigating {} on:", username.bright_green().bold());
        }
    }

    #[allow(dead_code)]
    pub fn print_found(&self, site: &str, url: &str) {
        let msg = if self.no_color {
            format!("[+] {}: {}", site, url)
        } else {
            format!("[{}] {}: {}", "+".bright_green(), site.bright_white(), url)
        };
        let _ = self.multi_progress.println(msg);
    }

    pub fn print_found_with_confidence(&self, site: &str, url: &str, status_tag: &str) {
        let msg = if self.no_color {
            format!("[+] {}: {} {}", site, url, status_tag)
        } else {
            format!(
                "[{}] {}: {} {}",
                "+".bright_green(),
                site.bright_white(),
                url,
                status_tag.bright_cyan()
            )
        };
        let _ = self.multi_progress.println(msg);
    }

    pub fn print_not_found(&self, site: &str) {
        if !self.verbose {
            return;
        }

        let msg = if self.no_color {
            format!("[-] {}: Not Found!", site)
        } else {
            format!(
                "[{}] {}: {}",
                "-".bright_red(),
                site,
                "Not Found!".bright_yellow()
            )
        };
        let _ = self.multi_progress.println(msg);
    }

    pub fn print_blocked(&self, site: &str, reason: &str) {
        let msg = if self.no_color {
            format!("[⊗] {}: BLOCKED: {}", site, reason)
        } else {
            format!(
                "[{}] {}: {}: {}",
                "⊗".bright_red().bold(),
                site.bright_white(),
                "BLOCKED".bright_red().bold(),
                reason.yellow()
            )
        };
        let _ = self.multi_progress.println(msg);
    }

    pub fn print_error(&self, site: &str, error: &str) {
        if !self.verbose {
            return;
        }

        let time = chrono::Local::now().format("%H:%M:%S").to_string();
        let msg = if self.no_color {
            format!("[{}] [!] {}: ERROR: {}", time, site, error)
        } else {
            format!(
                "[{}] [{}] {}: {}: {}",
                time.dimmed(),
                "!".bright_red(),
                site,
                "ERROR".bright_magenta(),
                error.bright_red()
            )
        };
        let _ = self.multi_progress.println(msg);
    }

    pub fn print_info(&self, message: &str) {
        let time = chrono::Local::now().format("%H:%M:%S").to_string();
        let msg = if self.no_color {
            format!("[{}] [*] {}", time, message)
        } else {
            format!("[{}] [{}] {}", time.dimmed(), "*".bright_blue(), message)
        };
        let _ = self.multi_progress.println(msg);
    }

    pub fn print_success(&self, message: &str) {
        let time = chrono::Local::now().format("%H:%M:%S").to_string();
        let msg = if self.no_color {
            format!("[{}] [✓] {}", time, message)
        } else {
            format!("[{}] [{}] {}", time.dimmed(), "✓".bright_green(), message.bright_white())
        };
        let _ = self.multi_progress.println(msg);
    }

    pub fn print_warning(&self, message: &str) {
        let time = chrono::Local::now().format("%H:%M:%S").to_string();
        let msg = if self.no_color {
            format!("[{}] [!] {}", time, message)
        } else {
            format!("[{}] [{}] {}", time.dimmed(), "!".bright_yellow(), message.yellow())
        };
        let _ = self.multi_progress.println(msg);
    }

    #[allow(dead_code)]
    pub fn create_progress_bar(&self, total: u64, message: &str) -> ProgressBar {
        let pb = self.multi_progress.add(ProgressBar::new(total));

        let style = if self.no_color {
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40} {pos}/{len} {msg}")
                .unwrap()
        } else {
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("█▓▒░")
        };

        pb.set_style(style);
        pb.set_message(message.to_string());
        pb
    }

    pub fn print_summary(&self, found: usize, total: usize, elapsed: std::time::Duration) {
        println!();
        if self.no_color {
            println!("═══════════════════════════════════════");
            println!("  SCAN COMPLETE");
            println!("═══════════════════════════════════════");
            println!("  Found:    {}", found);
            println!("  Checked:  {}", total);
            println!("  Time:     {:.2}s", elapsed.as_secs_f64());
            println!("═══════════════════════════════════════");
        } else {
            println!(
                "{}",
                "═══════════════════════════════════════".bright_cyan()
            );
            println!("  {}", "🧠 SCAN COMPLETE".bright_white().bold());
            println!(
                "{}",
                "═══════════════════════════════════════".bright_cyan()
            );
            println!(
                "  {}: {}",
                "Found".bright_green(),
                found.to_string().bright_white().bold()
            );
            println!(
                "  {}: {}",
                "Checked".bright_blue(),
                total.to_string().bright_white()
            );
            println!(
                "  {}: {}",
                "Time".bright_yellow(),
                format!("{:.2}s", elapsed.as_secs_f64()).bright_white()
            );
            println!(
                "{}",
                "═══════════════════════════════════════".bright_cyan()
            );
        }
    }

    pub fn print_intelligence_summary(
        &self,
        confirmed: usize,
        likely: usize,
        blocked: usize,
        stats: &ScraperStats,
    ) {
        if confirmed == 0 && likely == 0 && blocked == 0 {
            return;
        }

        println!();
        if self.no_color {
            println!("───────────────────────────────────────");
            println!("  INTELLIGENCE REPORT");
            println!("───────────────────────────────────────");
            if confirmed > 0 {
                println!("  Confirmed:  {}", confirmed);
            }
            if likely > 0 {
                println!("  Likely:     {}", likely);
            }
            if blocked > 0 {
                println!("  Blocked:    {}", blocked);
            }
            if stats.cloudflare_detected > 0 {
                println!("  Cloudflare: {}", stats.cloudflare_detected);
            }
            if let Some((site, duration)) = &stats.fastest_site {
                println!("  Fastest:    {} ({:.2}s)", site, duration.as_secs_f64());
            }
            if let Some((site, duration)) = &stats.slowest_site {
                println!("  Slowest:    {} ({:.2}s)", site, duration.as_secs_f64());
            }
            println!("───────────────────────────────────────");
        } else {
            println!(
                "{}",
                "───────────────────────────────────────".bright_magenta()
            );
            println!("  {}", "🎯 INTELLIGENCE REPORT".bright_white().bold());
            println!(
                "{}",
                "───────────────────────────────────────".bright_magenta()
            );
            if confirmed > 0 {
                println!(
                    "  {}: {}",
                    "Confirmed".bright_green().bold(),
                    confirmed.to_string().bright_white()
                );
            }
            if likely > 0 {
                println!(
                    "  {}: {}",
                    "Likely".bright_yellow(),
                    likely.to_string().bright_white()
                );
            }
            if blocked > 0 {
                println!(
                    "  {}: {}",
                    "Blocked".bright_red(),
                    blocked.to_string().bright_white()
                );
            }
            if stats.cloudflare_detected > 0 {
                println!(
                    "  {}: {}",
                    "Cloudflare".bright_cyan(),
                    stats.cloudflare_detected.to_string().bright_white()
                );
            }
            if let Some((site, duration)) = &stats.fastest_site {
                println!(
                    "  {}: {} ({})",
                    "Fastest".bright_green(),
                    site.bright_white(),
                    format!("{:.2}s", duration.as_secs_f64()).bright_cyan()
                );
            }
            if let Some((site, duration)) = &stats.slowest_site {
                println!(
                    "  {}: {} ({})",
                    "Slowest".bright_red(),
                    site.bright_white(),
                    format!("{:.2}s", duration.as_secs_f64()).bright_cyan()
                );
            }
            println!(
                "{}",
                "───────────────────────────────────────".bright_magenta()
            );
        }
    }
}

pub fn init_tracing(verbose: bool) {
    use tracing_subscriber::{fmt, EnvFilter};

    let filter = if verbose {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("info")
    };

    fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .init();
}
