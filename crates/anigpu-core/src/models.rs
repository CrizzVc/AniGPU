use serde::{Deserialize, Serialize};

/// Episode number: some sources report it as a number, others as text.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum EpisodeNum {
    Int(u32),
    Str(String),
}

impl EpisodeNum {
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            EpisodeNum::Int(n) => Some(*n as i64),
            EpisodeNum::Str(s) => s.trim().parse().ok(),
        }
    }
}

impl std::fmt::Display for EpisodeNum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EpisodeNum::Int(n) => n.fmt(f),
            EpisodeNum::Str(s) => f.write_str(s),
        }
    }
}

/// Item from `getLatest` (episodes feed on the home screen).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LatestItem {
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub episode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anime_url: Option<String>,
    pub url: String,
}

/// Item from `search` / `browse` (poster cards).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardItem {
    pub title: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anime_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodeItem {
    pub episode: EpisodeNum,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelatedItem {
    pub title: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimeDetails {
    pub title: String,
    pub synopsis: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backdrop: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default)]
    pub genres: Vec<String>,
    #[serde(default)]
    pub related: Vec<RelatedItem>,
    #[serde(default)]
    pub episodes: Vec<EpisodeItem>,
}

/// A play option on an episode page. `code` or `url` holds the embed link
/// to hand over to the extractor.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

impl ServerItem {
    /// The embed URL to extract, whichever field the source used.
    pub fn embed_url(&self) -> Option<&str> {
        self.code.as_deref().or(self.url.as_deref())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subtitle {
    pub file: String,
    pub label: String,
}

/// Result of running an embed URL through the providers.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Extraction {
    pub stream_url: String,
    #[serde(default = "yes")]
    pub is_direct: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    #[serde(default)]
    pub subtitles: Vec<Subtitle>,
    pub provider: String,
    pub original_url: String,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceInfo {
    pub id: String,
    pub name: String,
}
