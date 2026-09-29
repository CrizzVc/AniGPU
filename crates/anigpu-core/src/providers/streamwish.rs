use async_trait::async_trait;

use super::Provider;
use crate::extractors::{extract_m3u8_from_text, parse_jw_player};
use crate::http::get_text;
use crate::models::Extraction;
use crate::Result;

pub struct Streamwish;

pub static STREAMWISH: Streamwish = Streamwish;

#[async_trait]
impl Provider for Streamwish {
    fn name(&self) -> &'static str {
        "Streamwish"
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("streamwish") || url.contains("strwish") || url.contains("swish")
    }

    async fn extract(&self, url: &str) -> Result<Extraction> {
        let html = get_text(url).await?;

        // Try JWPlayer config first
        let jw = parse_jw_player(&html);
        if let Some(file) = jw.sources.first() {
            return Ok(Extraction {
                stream_url: file.clone(),
                is_direct: true,
                r#type: None,
                subtitles: jw.tracks,
                provider: String::new(),
                original_url: String::new(),
            });
        }

        // Fallback: raw m3u8 URL
        if let Some(m3u8) = extract_m3u8_from_text(&html) {
            return Ok(Extraction {
                stream_url: m3u8,
                is_direct: true,
                r#type: None,
                subtitles: Vec::new(),
                provider: String::new(),
                original_url: String::new(),
            });
        }

        Err(crate::error::Error::Extraction(
            "No stream found in Streamwish".into(),
        ))
    }
}
