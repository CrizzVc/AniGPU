use async_trait::async_trait;
use regex::Regex;
use std::sync::OnceLock;

use super::Provider;
use crate::http::get_text;
use crate::models::Extraction;
use crate::Result;

pub struct Maru;

pub static MARU: Maru = Maru;

fn data_options_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"data-options="([^"]+)""#).expect("valid regex"))
}

#[async_trait]
impl Provider for Maru {
    fn name(&self) -> &'static str {
        "Maru"
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("ok.ru") || url.contains("maru")
    }

    async fn extract(&self, url: &str) -> Result<Extraction> {
        let html = get_text(url).await?;

        if let Some(caps) = data_options_re().captures(&html) {
            let options_str = caps[1].replace("&quot;", "\"");
            if let Ok(options) = serde_json::from_str::<serde_json::Value>(&options_str) {
                if let Some(metadata_url) = options
                    .get("flashvars")
                    .and_then(|fv| fv.get("metadataUrl"))
                    .and_then(|v| v.as_str())
                {
                    let decoded_url = urlencoding::decode(metadata_url)
                        .unwrap_or_else(|_| metadata_url.into())
                        .to_string();

                    let meta_body = get_text(&decoded_url).await?;
                    if let Ok(meta_json) = serde_json::from_str::<serde_json::Value>(&meta_body) {
                        if let Some(hls_url) =
                            meta_json.get("hlsManifestUrl").and_then(|v| v.as_str())
                        {
                            return Ok(Extraction {
                                stream_url: hls_url.to_string(),
                                is_direct: true,
                                r#type: None,
                                subtitles: Vec::new(),
                                provider: String::new(),
                                original_url: String::new(),
                            });
                        }
                    }
                }
            }
        }

        Err(crate::error::Error::Extraction(
            "No stream found in Maru/Ok.ru".into(),
        ))
    }
}
