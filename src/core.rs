use anyhow::{Context, Result};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

fn deserialize_error_msg<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrVec {
        String(String),
        Vec(Vec<String>),
    }

    match StringOrVec::deserialize(deserializer)? {
        StringOrVec::String(s) => Ok(s),
        StringOrVec::Vec(v) => Ok(v.join("|")),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteData {
    #[serde(rename = "errorType")]
    pub error_type: String,

    #[serde(
        rename = "errorMsg",
        default,
        deserialize_with = "deserialize_error_msg"
    )]
    pub error_msg: String,

    pub url: String,

    #[serde(rename = "urlMain")]
    pub url_main: String,

    #[serde(rename = "urlProbe", default)]
    pub url_probe: String,

    #[serde(rename = "errorUrl", default)]
    pub error_url: String,

    #[serde(rename = "username_claimed")]
    pub username_claimed: String,

    #[serde(rename = "username_unclaimed")]
    pub username_unclaimed: String,

    #[serde(rename = "regexCheck", default)]
    pub regex_check: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ResultStatus {
    Confirmed,
    Likely,
    Private,
    NotFound,
    Blocked,
    Soft404,
    Redirected,
    Error,
}

impl ResultStatus {
    pub fn as_tag(&self) -> &'static str {
        match self {
            ResultStatus::Confirmed => "CONFIRMED",
            ResultStatus::Likely => "LIKELY",
            ResultStatus::Private => "PRIVATE",
            ResultStatus::NotFound => "NOT_FOUND",
            ResultStatus::Blocked => "BLOCKED",
            ResultStatus::Soft404 => "SOFT_404",
            ResultStatus::Redirected => "REDIRECTED",
            ResultStatus::Error => "ERROR",
        }
    }

    #[allow(dead_code)]
    pub fn is_found(&self) -> bool {
        matches!(
            self,
            ResultStatus::Confirmed | ResultStatus::Likely | ResultStatus::Private
        )
    }
}

impl Serialize for ResultStatus {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_tag())
    }
}

pub type ConfidenceScore = f32;

#[derive(Debug, Clone, Serialize)]
pub struct ScanResult {
    pub username: String,
    pub site: String,
    pub url: String,
    pub url_probe: String,
    pub link: String,
    pub exist: bool,
    pub proxied: bool,
    pub error: bool,
    pub error_msg: String,
    pub status: ResultStatus,
    pub confidence: ConfidenceScore,
    pub page_title: Option<String>,
    pub page_bio: Option<String>,
}

impl ScanResult {
    pub fn new(username: String, site: String) -> Self {
        Self {
            username,
            site,
            url: String::new(),
            url_probe: String::new(),
            link: String::new(),
            exist: false,
            proxied: false,
            error: false,
            error_msg: String::new(),
            status: ResultStatus::NotFound,
            confidence: 0.0,
            page_title: None,
            page_bio: None,
        }
    }

    pub fn with_error(mut self, error_msg: String, status: ResultStatus) -> Self {
        self.error = true;
        self.error_msg = error_msg;
        self.status = status;
        self.confidence = 0.0;
        self
    }

    pub fn found(
        mut self,
        url: String,
        link: String,
        status: ResultStatus,
        confidence: ConfidenceScore,
        page_title: Option<String>,
        page_bio: Option<String>,
    ) -> Self {
        self.exist = true;
        self.url = url;
        self.link = link;
        self.status = status;
        self.confidence = confidence.clamp(0.0, 1.0);
        self.page_title = page_title;
        self.page_bio = page_bio;
        self
    }

    pub fn not_found(
        mut self,
        url: String,
        status: ResultStatus,
        confidence: ConfidenceScore,
    ) -> Self {
        self.exist = false;
        self.url = url;
        self.status = status;
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    pub fn status_tag(&self) -> String {
        if self.confidence > 0.0 {
            format!(
                "[{}] (confidence: {:.2})",
                self.status.as_tag(),
                self.confidence
            )
        } else {
            format!("[{}]", self.status.as_tag())
        }
    }
}

pub type SiteDatabase = HashMap<String, SiteData>;

pub async fn load_site_data(path: &str, update: bool) -> Result<SiteDatabase> {
    if update || !Path::new(path).exists() {
        update_database(path).await?;
    }

    let file = std::fs::File::open(path)
        .with_context(|| format!("Failed to open database file: {}", path))?;
    let reader = std::io::BufReader::new(file);

    let data: SiteDatabase =
        serde_json::from_reader(reader).with_context(|| "Failed to parse database JSON")?;

    Ok(data)
}

async fn update_database(path: &str) -> Result<()> {
    use colored::Colorize;

    println!(
        "[{}] Update database: {}",
        "!".bright_blue(),
        "Downloading...".bright_yellow()
    );

    let url = "https://raw.githubusercontent.com/sherlock-project/sherlock/master/sherlock_project/resources/data.json";

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()?;

    let response = client.get(url).send().await?;

    if !response.status().is_success() {
        anyhow::bail!("Failed to download database: HTTP {}", response.status());
    }

    let content = response.text().await?;

    if Path::new(path).exists() {
        fs::remove_file(path)?;
    }

    fs::write(path, content)?;

    println!(" [{}]", "Done".green());

    Ok(())
}

pub fn filter_sites(database: &SiteDatabase, site_filter: Option<&str>) -> SiteDatabase {
    if let Some(site_name) = site_filter {
        let site_lower = site_name.to_lowercase();
        database
            .iter()
            .filter(|(name, _)| name.to_lowercase() == site_lower)
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    } else {
        database.clone()
    }
}
