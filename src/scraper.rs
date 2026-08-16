use crate::core::{ResultStatus, ScanResult, SiteData};
use anyhow::Result;
use fancy_regex::Regex;
use reqwest::{Client, Proxy, Response};
use std::sync::Arc;
use std::time::{Duration, Instant};

const TOR_PROXY: &str = "socks5://127.0.0.1:9050";

const USER_AGENTS: &[&str] = &[
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScrapingStrategy {
    Fast,
    Stealth,
    AntiBlock,
}

#[derive(Debug, Clone)]
pub struct ScraperStats {
    pub total_requests: usize,
    pub blocked_count: usize,
    pub cloudflare_detected: usize,
    pub fastest_site: Option<(String, Duration)>,
    pub slowest_site: Option<(String, Duration)>,
}

impl ScraperStats {
    pub fn new() -> Self {
        Self {
            total_requests: 0,
            blocked_count: 0,
            cloudflare_detected: 0,
            fastest_site: None,
            slowest_site: None,
        }
    }

    pub fn update_timing(&mut self, site: String, duration: Duration) {
        if self.fastest_site.is_none() || duration < self.fastest_site.as_ref().unwrap().1 {
            self.fastest_site = Some((site.clone(), duration));
        }
        if self.slowest_site.is_none() || duration > self.slowest_site.as_ref().unwrap().1 {
            self.slowest_site = Some((site, duration));
        }
    }
}

pub struct IntelligentScraper {
    client: Arc<Client>,
    tor_client: Option<Arc<Client>>,
    proxy_clients: Vec<Arc<Client>>,
    jitter: Option<u64>,
    stats: Arc<std::sync::Mutex<ScraperStats>>,
}

impl IntelligentScraper {
    pub fn new(use_tor: bool, proxy_list: Vec<String>, timeout_secs: u64, jitter: Option<u64>) -> Result<Self> {
        let client = Arc::new(
            Client::builder()
                .user_agent(USER_AGENTS[0])
                .timeout(Duration::from_secs(timeout_secs))
                .connect_timeout(Duration::from_secs(timeout_secs / 2 + 1))
                .redirect(reqwest::redirect::Policy::limited(5))
                .pool_max_idle_per_host(20)
                .pool_idle_timeout(Duration::from_secs(90))
                .tcp_keepalive(Duration::from_secs(60))
                .http2_prior_knowledge()
                .build()?,
        );

        let tor_client = if use_tor {
            let proxy = Proxy::all(TOR_PROXY)?;
            Some(Arc::new(
                Client::builder()
                    .user_agent(USER_AGENTS[0])
                    .timeout(Duration::from_secs(timeout_secs + 20))
                    .proxy(proxy)
                    .redirect(reqwest::redirect::Policy::limited(5))
                    .build()?,
            ))
        } else {
            None
        };

        let mut proxy_clients = Vec::new();
        for proxy_url in proxy_list {
            if let Ok(proxy) = Proxy::all(&proxy_url) {
                if let Ok(client) = Client::builder()
                    .user_agent(USER_AGENTS[0])
                    .timeout(Duration::from_secs(timeout_secs))
                    .proxy(proxy)
                    .redirect(reqwest::redirect::Policy::limited(5))
                    .build()
                {
                    proxy_clients.push(Arc::new(client));
                }
            }
        }

        Ok(Self {
            client,
            tor_client,
            proxy_clients,
            jitter,
            stats: Arc::new(std::sync::Mutex::new(ScraperStats::new())),
        })
    }

    pub fn get_stats(&self) -> ScraperStats {
        self.stats.lock().unwrap().clone()
    }

    fn get_random_user_agent(&self) -> &'static str {
        let idx = fastrand::usize(0..USER_AGENTS.len());
        USER_AGENTS[idx]
    }

    fn detect_cloudflare(&self, response: &Response) -> bool {
        if let Some(server) = response.headers().get("server") {
            if let Ok(server_str) = server.to_str() {
                if server_str.to_lowercase().contains("cloudflare") {
                    return true;
                }
            }
        }
        response.headers().get("cf-ray").is_some()
    }

    fn detect_rate_limit(&self, response: &Response) -> bool {
        let status = response.status().as_u16();
        matches!(status, 429 | 503)
    }

    fn quick_html_check(&self, body: &str) -> (bool, f32) {
        let body_lower = body.to_lowercase();

        if body_lower.contains("page not found")
            || body_lower.contains("404")
            || body_lower.contains("user not found")
        {
            return (false, 0.9);
        }

        let positive_count = ["profile", "posts", "followers"]
            .iter()
            .filter(|&p| body_lower.contains(p))
            .count();

        if positive_count >= 2 {
            (true, 0.85)
        } else {
            (false, 0.5)
        }
    }

    fn extract_metadata(&self, body: &str) -> (Option<String>, Option<String>) {
        let title_re = Regex::new(r"(?i)<title[^>]*>\s*(.*?)\s*</title>").unwrap();
        
        let title = match title_re.captures(body) {
            Ok(Some(cap)) => cap.get(1).map(|m| m.as_str().to_string()),
            _ => None,
        };

        let mut bio = None;
        let meta_tags_re = Regex::new(r"(?i)<meta\s+([^>]+)>").unwrap();
        let content_re = Regex::new(r#"(?i)content=['"]([^'"]+)['"]"#).unwrap();
        
        for cap_res in meta_tags_re.captures_iter(body) {
            if let Ok(cap) = cap_res {
                let attrs = cap.get(1).map_or("", |m| m.as_str());
                let attrs_lower = attrs.to_lowercase();
                if attrs_lower.contains("name=\"description\"") || attrs_lower.contains("name='description'") {
                    if let Ok(Some(content_cap)) = content_re.captures(attrs) {
                        bio = content_cap.get(1).map(|m| m.as_str().to_string());
                        break;
                    }
                }
            }
        }

        (title, bio)
    }

    pub async fn check_username_intelligent(
        &self,
        username: &str,
        site: &str,
        data: &SiteData,
        use_tor: bool,
        strategy: ScrapingStrategy,
    ) -> ScanResult {
        let start_time = Instant::now();
        let mut result = ScanResult::new(username.to_string(), site.to_string());
        result.proxied = use_tor;

        let url = data.url.replace("{}", username);
        let url_probe = if !data.url_probe.is_empty() {
            data.url_probe.replace("{}", username)
        } else {
            url.clone()
        };

        result.url = data.url.clone();
        result.url_probe = data.url_probe.clone();

        if !data.regex_check.is_empty() {
            if let Ok(re) = Regex::new(&data.regex_check) {
                if let Ok(is_match) = re.is_match(username) {
                    if !is_match {
                        return result.not_found(url, ResultStatus::NotFound, 0.95);
                    }
                }
            }
        }

        if let Some(j) = self.jitter {
            tokio::time::sleep(Duration::from_millis(fastrand::u64(0..=j))).await;
        }

        let client = if use_tor && self.tor_client.is_some() {
            self.tor_client.as_ref().unwrap()
        } else if !self.proxy_clients.is_empty() {
            let idx = fastrand::usize(0..self.proxy_clients.len());
            &self.proxy_clients[idx]
        } else {
            &self.client
        };

        let mut request = client.get(&url_probe);

        if strategy != ScrapingStrategy::Fast {
            request = request.header("User-Agent", self.get_random_user_agent());
        }

        let response = match request.send().await {
            Ok(resp) => resp,
            Err(e) => {
                self.stats.lock().unwrap().total_requests += 1;
                return result.with_error(e.to_string(), ResultStatus::Error);
            }
        };

        {
            let mut stats = self.stats.lock().unwrap();
            stats.total_requests += 1;

            if self.detect_cloudflare(&response) {
                stats.cloudflare_detected += 1;
            }

            if self.detect_rate_limit(&response) {
                stats.blocked_count += 1;
                return result.with_error("Rate limited".to_string(), ResultStatus::Blocked);
            }
        }

        let status_code = response.status();
        let final_url = response.url().to_string();
        
        let body = match response.text().await {
            Ok(text) => text,
            Err(e) => return result.with_error(e.to_string(), ResultStatus::Error),
        };

        let (exists, confidence, status) = match data.error_type.as_str() {
            "status_code" => {
                if status_code.is_success() {
                    (true, 0.85, ResultStatus::Confirmed)
                } else if status_code.as_u16() == 404 {
                    (false, 0.90, ResultStatus::NotFound)
                } else {
                    (false, 0.60, ResultStatus::NotFound)
                }
            }
            "message" => {
                let has_error_msg = body.contains(&data.error_msg);

                if !has_error_msg {
                    let (html_exists, html_conf) = self.quick_html_check(&body);
                    if html_exists {
                        (true, html_conf, ResultStatus::Confirmed)
                    } else {
                        (true, 0.75, ResultStatus::Likely)
                    }
                } else {
                    (false, 0.90, ResultStatus::NotFound)
                }
            }
            "response_url" => {
                if status_code.is_success() && final_url == url {
                    (true, 0.90, ResultStatus::Confirmed)
                } else {
                    (false, 0.85, ResultStatus::NotFound)
                }
            }
            _ => {
                return result.with_error(
                    format!("Unsupported error type: {}", data.error_type),
                    ResultStatus::Error,
                );
            }
        };

        let elapsed = start_time.elapsed();
        self.stats
            .lock()
            .unwrap()
            .update_timing(site.to_string(), elapsed);

        if exists {
            let (page_title, page_bio) = self.extract_metadata(&body);
            result.found(url.clone(), url.clone(), status, confidence, page_title, page_bio)
        } else {
            result.not_found(url, status, confidence)
        }
    }
}

pub async fn check_with_adaptive_strategy(
    scraper: &IntelligentScraper,
    username: &str,
    site: &str,
    data: &SiteData,
    use_tor: bool,
    max_retries: u32,
) -> ScanResult {
    let mut retries = 0;
    let mut current_strategy = ScrapingStrategy::Fast;

    loop {
        let result = scraper
            .check_username_intelligent(username, site, data, use_tor, current_strategy)
            .await;

        if !result.error || result.status == ResultStatus::Blocked || retries >= max_retries {
            return result;
        }

        retries += 1;

        current_strategy = match retries {
            1 => ScrapingStrategy::Stealth,
            _ => ScrapingStrategy::AntiBlock,
        };

        tokio::time::sleep(Duration::from_millis(100 * retries as u64)).await;
    }
}
