pub mod filemoon;
pub mod maru;
pub mod mp4upload;
pub mod streamwish;
pub mod yourupload;

use async_trait::async_trait;

use crate::models::Extraction;
use crate::Result;

/// A video host extractor. Implementations are stateless.
#[async_trait]
pub trait Provider: Send + Sync {
    fn name(&self) -> &'static str;
    fn can_handle(&self, url: &str) -> bool;
    async fn extract(&self, url: &str) -> Result<Extraction>;
}

/// All registered providers in priority order.
static ALL_PROVIDERS: &[&dyn Provider] = &[
    &streamwish::STREAMWISH,
    &filemoon::FILEMOON,
    &yourupload::YOURUPLOAD,
    &maru::MARU,
    &mp4upload::MP4UPLOAD,
];

/// Find the first provider that can handle `url` and run its extraction.
/// Mirrors the behaviour of `animeProvider.extract(url)` from the Node backend.
pub async fn extract(url: &str) -> Result<Extraction> {
    for provider in ALL_PROVIDERS {
        if provider.can_handle(url) {
            tracing::info!(
                "[AnimeProvider] Using extractor: {} for {}",
                provider.name(),
                url
            );
            match provider.extract(url).await {
                Ok(mut result) => {
                    result.provider = provider.name().to_string();
                    result.original_url = url.to_string();
                    return Ok(result);
                }
                Err(e) => {
                    tracing::warn!(
                        "[AnimeProvider] Extraction failed with {}: {}",
                        provider.name(),
                        e
                    );
                    return Err(e);
                }
            }
        }
    }

    tracing::info!("[AnimeProvider] No extractor supports: {}", url);
    Err(crate::error::Error::UnsupportedProvider(url.to_string()))
}
