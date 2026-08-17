mod chrome;
mod cli;
mod core;
mod downloader;
mod export;
mod logger;
mod scraper;
mod tui;
mod variants;

use anyhow::{Context, Result};
use cli::Cli;
use colored::Colorize;
use core::{filter_sites, load_site_data, ResultStatus};
use downloader::DownloaderRegistry;
use logger::Logger;
use scraper::{check_with_adaptive_strategy, IntelligentScraper};
use serde::Serialize;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Semaphore;
use tracing::info;

#[derive(Debug, Serialize)]
pub struct UsernameScanReport {
    pub username: String,
    pub checked: usize,
    pub found: usize,
    pub confirmed: usize,
    pub likely: usize,
    pub blocked: usize,
    pub elapsed_secs: f64,
    pub results: Vec<core::ScanResult>,
}

#[derive(Debug, Serialize)]
pub struct ScanReport {
    pub generated_at: String,
    pub database_sites: usize,
    pub usernames: Vec<UsernameScanReport>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Cli::parse_args();

    logger::init_tracing(args.verbose);

    print_banner();

    if args.download && args.usernames.is_empty() {
        let registry = DownloaderRegistry::new();
        println!("List of sites that can download userdata:");
        for site in registry.list_available() {
            if args.no_color {
                println!("[+] {}", site);
            } else {
                println!("[{}] {}", "+".bright_green(), site.bright_white());
            }
        }
        return Ok(());
    }

    let database = load_site_data(&args.database_path(), args.update).await?;
    info!(
        "[core] Loaded {} sites from database",
        database.len().to_string().bright_cyan()
    );

    if args.test {
        run_tests(&args, &database).await?;
        return Ok(());
    }

    let mut target_usernames = args.usernames.clone();
    
    // Read usernames from file if provided
    if let Some(input_path) = &args.input {
        let content = std::fs::read_to_string(input_path)
            .with_context(|| format!("Failed to read input file: {}", input_path))?;
        for line in content.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                target_usernames.push(trimmed.to_string());
            }
        }
    }

    // ── Variant Generation ────────────────────────────────────────────────
    if args.variants && !target_usernames.is_empty() {
        let mut expanded = Vec::new();
        for base in &target_usernames {
            let vars = variants::generate_variants(base);
            variants::print_variant_summary(base, &vars, args.no_color);
            expanded.push(base.clone());
            expanded.extend(vars);
        }
        target_usernames = expanded;
    }
    
    if target_usernames.is_empty() {
        println!("Error: No usernames provided. Use positional arguments or --input <FILE>.");
        return Ok(());
    }

    if args.tui {
        if target_usernames.is_empty() {
            println!("Error: No usernames provided for TUI.");
            return Ok(());
        }
        
        let username = target_usernames[0].clone();
        let username_clone = username.clone();
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        
        let db_clone = database.clone();
        let args_clone = args.clone();
        
        let _handle = tokio::spawn(async move {
            let _ = scan_username(&username_clone, &args_clone, &db_clone, Some(tx.clone())).await;
            let _ = tx.send(tui::AppEvent::Done);
        });

        let app = tui::App::new(username);
        tui::run_tui(app, rx)?;
        return Ok(());
    }

    let mut reports = Vec::new();

    for username in &target_usernames {
        reports.push(scan_username(username, &args, &database, None).await?);
    }

    if let Some(output_path) = &args.output {
        let format = args.output_format();
        export::write_report(output_path, database.len(), reports, format)?;
    }

    Ok(())
}

async fn scan_username(
    username: &str,
    args: &Cli,
    database: &core::SiteDatabase,
    tui_tx: Option<tokio::sync::mpsc::UnboundedSender<tui::AppEvent>>,
) -> Result<UsernameScanReport> {
    let logger = Logger::new(args.no_color, args.verbose);
    if tui_tx.is_none() {
        logger.print_banner(username);
    }

    let sites = filter_sites(database, args.site.as_deref());
    let start_time = Instant::now();

    if sites.is_empty() {
        logger.print_error("site", "No matching sites found");
        return Ok(UsernameScanReport {
            username: username.to_string(),
            checked: 0,
            found: 0,
            confirmed: 0,
            likely: 0,
            blocked: 0,
            elapsed_secs: start_time.elapsed().as_secs_f64(),
            results: Vec::new(),
        });
    }

    let proxy_pool = if let Some(path) = &args.proxies {
        std::fs::read_to_string(path)
            .unwrap_or_default()
            .lines()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    } else {
        Vec::new()
    };
    let scraper = Arc::new(IntelligentScraper::new(args.tor, proxy_pool, args.timeout, args.jitter)?);

    let chrome = if args.screenshot {
        let mut chrome = chrome::Chrome::new(
            "1024x768".to_string(),
            60,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36".to_string(),
        );
        chrome.setup()?;
        Some(Arc::new(chrome))
    } else {
        None
    };

    let downloader_registry = if args.download {
        Some(Arc::new(DownloaderRegistry::new()))
    } else {
        None
    };

    let semaphore = Arc::new(Semaphore::new(args.max_workers()));

    let found_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let confirmed_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let likely_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let blocked_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut tasks = Vec::new();

    let pb = logger.create_progress_bar(sites.len() as u64, &format!("Scanning {}...", username));

    for (site_name, site_data) in sites.iter() {
        let username = username.to_string();
        let site_name = site_name.clone();
        let site_data = site_data.clone();
        let scraper = Arc::clone(&scraper);
        let semaphore = Arc::clone(&semaphore);
        let found_count = Arc::clone(&found_count);
        let confirmed_count = Arc::clone(&confirmed_count);
        let likely_count = Arc::clone(&likely_count);
        let blocked_count = Arc::clone(&blocked_count);
        let chrome = chrome.clone();
        let downloader_registry = downloader_registry.clone();
        let args = args.clone();
        let logger = logger.clone();
        let pb = pb.clone();
        let tui_tx = tui_tx.clone();
        let total_sites = sites.len();

        let task = tokio::spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();

            let result = check_with_adaptive_strategy(
                &scraper, &username, &site_name, &site_data, args.tor, 2,
            )
            .await;

            match result.status {
                ResultStatus::Confirmed => {
                    confirmed_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                }
                ResultStatus::Likely => {
                    likely_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                }
                ResultStatus::Blocked => {
                    blocked_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                }
                _ => {}
            }

            if result.exist {
                found_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                
                if let Some(tx) = &tui_tx {
                    let _ = tx.send(tui::AppEvent::Result(result.clone()));
                    let _ = tx.send(tui::AppEvent::Log(format!("[+] Found on {}", site_name)));
                } else {
                    logger.print_found_with_confidence(
                        &site_name, 
                        &result.link, 
                        &result.status_tag(),
                        result.page_title.as_deref(),
                        result.page_bio.as_deref(),
                    );
                }

                if let Some(chrome) = chrome {
                    if let Err(e) =
                        chrome::take_screenshot(&username, &site_name, &result.link, &chrome)
                    {
                        if tui_tx.is_none() {
                            logger.print_warning(&format!("Screenshot failed for {}: {}", site_name, e));
                        }
                    }
                }

                if let Some(registry) = downloader_registry {
                    if let Err(e) = registry.download(&site_name, &result.link, &username).await {
                        if tui_tx.is_none() {
                            logger.print_warning(&format!("Download failed for {}: {}", site_name, e));
                        }
                    }
                }
            } else if result.error {
                if let Some(tx) = &tui_tx {
                    let _ = tx.send(tui::AppEvent::Log(format!("[!] Error on {}: {}", site_name, result.error_msg)));
                } else {
                    if result.status == ResultStatus::Blocked {
                        logger.print_blocked(&site_name, &result.error_msg);
                    } else {
                        logger.print_error(&site_name, &result.error_msg);
                    }
                }
            } else if args.verbose {
                if tui_tx.is_none() {
                    logger.print_not_found(&site_name);
                }
            }

            pb.inc(1);
            if let Some(tx) = &tui_tx {
                let current = pb.position() as usize;
                let _ = tx.send(tui::AppEvent::Progress { current, total: total_sites });
            }
            
            result
        });

        tasks.push(task);
    }

    let mut results = Vec::with_capacity(tasks.len());
    for task in tasks {
        if let Ok(result) = task.await {
            results.push(result);
        }
    }

    pb.finish_and_clear();

    let stats = scraper.get_stats();

    let elapsed = start_time.elapsed();
    let found = found_count.load(std::sync::atomic::Ordering::SeqCst);
    let confirmed = confirmed_count.load(std::sync::atomic::Ordering::SeqCst);
    let likely = likely_count.load(std::sync::atomic::Ordering::SeqCst);
    let blocked = blocked_count.load(std::sync::atomic::Ordering::SeqCst);

    if tui_tx.is_none() {
        logger.print_summary(found, sites.len(), elapsed);
        logger.print_intelligence_summary(confirmed, likely, blocked, &stats);
    }

    Ok(UsernameScanReport {
        username: username.to_string(),
        checked: sites.len(),
        found,
        confirmed,
        likely,
        blocked,
        elapsed_secs: elapsed.as_secs_f64(),
        results,
    })
}

async fn run_tests(args: &Cli, database: &core::SiteDatabase) -> Result<()> {
    let logger = Logger::new(args.no_color, args.verbose);

    logger.print_info("vesper is activated for checking site validity.");

    if args.screenshot {
        logger.print_warning("Taking screenshot is not available in test mode. Aborted.");
        return Ok(());
    }

    let proxy_pool = if let Some(path) = &args.proxies {
        std::fs::read_to_string(path)
            .unwrap_or_default()
            .lines()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    } else {
        Vec::new()
    };
    let scraper = Arc::new(IntelligentScraper::new(args.tor, proxy_pool, args.timeout, args.jitter)?);
    let semaphore = Arc::new(Semaphore::new(args.max_workers()));
    let failed_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let mut tasks = Vec::new();

    for (site_name, site_data) in database.iter() {
        let site_name = site_name.clone();
        let site_data = site_data.clone();
        let scraper = Arc::clone(&scraper);
        let semaphore = Arc::clone(&semaphore);
        let failed_count = Arc::clone(&failed_count);
        let logger = logger.clone();
        let use_tor = args.tor;

        let task = tokio::spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();

            let used_result = scraper
                .check_username_intelligent(
                    &site_data.username_claimed,
                    &site_name,
                    &site_data,
                    use_tor,
                    scraper::ScrapingStrategy::Fast,
                )
                .await;

            let unused_result = scraper
                .check_username_intelligent(
                    &site_data.username_unclaimed,
                    &site_name,
                    &site_data,
                    use_tor,
                    scraper::ScrapingStrategy::Fast,
                )
                .await;

            if used_result.exist && !unused_result.exist {
            } else {
                failed_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

                let mut error_msg = String::new();
                if used_result.error {
                    error_msg.push_str(&format!("[{}]", used_result.error_msg));
                }
                if unused_result.error {
                    error_msg.push_str(&format!("[{}]", unused_result.error_msg));
                }

                if !error_msg.is_empty() {
                    logger.print_error(&site_name, &format!("Failed with error {}", error_msg));
                } else {
                    let msg = format!(
                        "Not working ({}: expected true, but {}, {}: expected false, but {})",
                        site_data.username_claimed,
                        used_result.exist,
                        site_data.username_unclaimed,
                        unused_result.exist
                    );
                    logger.print_warning(&format!("{}: {}", site_name, msg));
                }
            }
        });

        tasks.push(task);
    }

    for task in tasks {
        let _ = task.await;
    }

    logger.print_success("Done");

    let failed = failed_count.load(std::sync::atomic::Ordering::SeqCst);
    println!(
        "\nThese {} sites are not compatible with the Sherlock database.",
        failed
    );

    Ok(())
}

fn print_banner() {
    println!(
        r#"
    ██╗   ██╗ ███████╗ ██████╗ ██████╗ ███████╗ ██████╗ 
    ██║   ██║ ██╔════╝██╔════╝ ██╔══██╗ ██╔════╝ ██╔══██╗
    ██║   ██║ █████╗  ╚█████╗  ██████╔╝ █████╗  ██████╔╝
    ╚██╗ ██╔╝ ██╔══╝   ╚═══██╗ ██╔═══╝  ██╔══╝  ██╔══██╗
     ╚████╔╝  ███████╗ ██████╔╝ ██║      ███████╗ ██║  ██║
      ╚═══╝   ╚══════╝ ╚═════╝  ╚═╝      ╚══════╝ ╚═╝  ╚═╝
    
    🔎 vesper — Professional OSINT Username Scanner
    "#
    );
}
