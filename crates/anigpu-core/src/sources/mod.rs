pub mod animeav1;
pub mod animeflv;
pub mod animeonlineninja;
pub mod jkanime;

use async_trait::async_trait;

use crate::models::{AnimeDetails, CardItem, LatestItem, ServerItem, SourceInfo};
use crate::Result;

/// A site scraper. Implementations are stateless singletons.
#[async_trait]
pub trait Source: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;

    async fn get_latest(&self) -> Result<Vec<LatestItem>>;
    async fn get_details(&self, url: &str) -> Result<AnimeDetails>;
    async fn get_servers(&self, url: &str) -> Result<Vec<ServerItem>>;
    async fn search(&self, query: &str) -> Result<Vec<CardItem>>;
    async fn browse(&self, page: u32) -> Result<Vec<CardItem>>;
}

/// Resolve a source id, defaulting to `animeav1` (same behaviour as the old `sources/index.js`).
pub fn get_source(id: Option<&str>) -> &'static dyn Source {
    let id = id.unwrap_or_default();
    match id {
        "jkanime" => &jkanime::JKANIME,
        "animeflv" => &animeflv::ANIMEFLV,
        "animeonlineninja" => &animeonlineninja::ANIMEONLINENINJA,
        _ => &animeav1::ANIMEAV1,
    }
}

pub fn all_sources() -> Vec<SourceInfo> {
    [
        &animeav1::ANIMEAV1 as &dyn Source,
        &jkanime::JKANIME,
        &animeflv::ANIMEFLV,
        &animeonlineninja::ANIMEONLINENINJA,
    ]
    .iter()
    .map(|s| SourceInfo {
        id: s.id().to_string(),
        name: s.name().to_string(),
    })
    .collect()
}
