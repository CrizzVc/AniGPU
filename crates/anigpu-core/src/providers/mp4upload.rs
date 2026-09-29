use async_trait::async_trait;
use regex::Regex;
use std::sync::OnceLock;

use super::Provider;
use crate::http::get_text;
use crate::models::Extraction;
use crate::Result;

pub struct Mp4Upload;

pub static MP4UPLOAD: Mp4Upload = Mp4Upload;

fn src_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"src:\s*"(https://.*?\.mp4)""#).expect("valid regex"))
}

fn player_src_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"script[\s\S]*?player\.src\("(.*?)"\)"#).expect("valid regex")
    })
}

#[async_trait]
impl Provider for Mp4Upload {
    fn name(&self) -> &'static str {
        "MP4Upload"
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("mp4upload.com")
    }

    async fn extract(&self, url: &str) -> Result<Extraction> {
        let embed_url = if url.contains("embed-") {
            url.to_string()
        } else {
            url.replace(".com/", ".com/embed-") + ".html"
        };

        let html = get_text(&embed_url).await?;

        // Primary: src: "https://...mp4"
        if let Some(caps) = src_re().captures(&html) {
            return Ok(Extraction {
                stream_url: caps[1].to_string(),
                is_direct: true,
                r#type: Some("mp4".into()),
                subtitles: Vec::new(),
                provider: String::new(),
                original_url: String::new(),
            });
        }

        // Fallback: player.src("...")
        if let Some(caps) = player_src_re().captures(&html) {
            return Ok(Extraction {
                stream_url: caps[1].to_string(),
                is_direct: true,
                r#type: Some("mp4".into()),
                subtitles: Vec::new(),
                provider: String::new(),
                original_url: String::new(),
            });
        }

        Err(crate::error::Error::Extraction(
            "Could not find video source in MP4Upload".into(),
        ))
    }
}
