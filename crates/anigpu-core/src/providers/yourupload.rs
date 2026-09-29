use async_trait::async_trait;
use regex::Regex;
use std::sync::OnceLock;

use super::Provider;
use crate::extractors::extract_mp4_from_text;
use crate::http::get_text_with;
use crate::models::Extraction;
use crate::Result;

pub struct YourUpload;

pub static YOURUPLOAD: YourUpload = YourUpload;

fn og_video_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"property="og:video"\s*content="([^"]+)""#).expect("valid regex")
    })
}

#[async_trait]
impl Provider for YourUpload {
    fn name(&self) -> &'static str {
        "YourUpload"
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("yourupload")
    }

    async fn extract(&self, url: &str) -> Result<Extraction> {
        let embed_url = if url.contains("/watch/") {
            url.replace("/watch/", "/embed/")
        } else {
            url.to_string()
        };

        let html = get_text_with(
            &embed_url,
            &[("Referer", "https://www.yourupload.com/")],
        )
        .await?;

        // Try og:video meta tag
        if let Some(caps) = og_video_re().captures(&html) {
            return Ok(Extraction {
                stream_url: caps[1].to_string(),
                is_direct: true,
                r#type: None,
                subtitles: Vec::new(),
                provider: String::new(),
                original_url: String::new(),
            });
        }

        // Fallback: raw mp4 URL
        if let Some(mp4) = extract_mp4_from_text(&html) {
            return Ok(Extraction {
                stream_url: mp4,
                is_direct: true,
                r#type: None,
                subtitles: Vec::new(),
                provider: String::new(),
                original_url: String::new(),
            });
        }

        Err(crate::error::Error::Extraction(
            "No stream found in YourUpload".into(),
        ))
    }
}
