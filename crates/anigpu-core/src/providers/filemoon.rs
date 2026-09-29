use async_trait::async_trait;

use super::Provider;
use crate::extractors::{extract_m3u8_from_text, extract_packed};
use crate::http::get_text;
use crate::models::Extraction;
use crate::Result;

pub struct Filemoon;

pub static FILEMOON: Filemoon = Filemoon;

#[async_trait]
impl Provider for Filemoon {
    fn name(&self) -> &'static str {
        "Filemoon"
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("filemoon") || url.contains("fmoon")
    }

    async fn extract(&self, url: &str) -> Result<Extraction> {
        let html = get_text(url).await?;

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

        let packed = extract_packed(&html);
        if !packed.is_empty() {
            tracing::info!(
                "Filemoon stream is packed ({} scripts). Real extraction requires unpacker.",
                packed.len()
            );
        }

        Err(crate::error::Error::Extraction(
            "No stream found in Filemoon or stream is obfuscated.".into(),
        ))
    }
}
