use serde::Deserialize;
use crate::http::get_text;
use crate::Result;

#[derive(Deserialize)]
struct TmdbSearchResponse {
    results: Vec<TmdbAnime>,
}

#[derive(Deserialize)]
struct TmdbAnime {
    id: u32,
    poster_path: Option<String>,
    backdrop_path: Option<String>,
}

pub struct Tmdb;

impl Tmdb {
    const BASE_URL: &'static str = "https://api.themoviedb.org/3";
    const IMAGE_BASE: &'static str = "https://image.tmdb.org/t/p/original";

    pub async fn find_anime_images(title: &str) -> Result<(Option<String>, Option<String>)> {
        // In a real scenario, the API key would come from config/env
        let api_key = std::env::var("TMDB_API_KEY").unwrap_or_else(|_| "YOUR_TMDB_API_KEY".to_string());

        let url = format!(
            "{} /search/tv?api_key={}&query={}&language=es-ES",
            Self::BASE_URL,
            api_key,
            urlencoding::encode(title)
        ).replace(" ", "");

        let html = get_text(&url).await?;
        // Note: get_text usually returns the body as a string.
        // Since TMDB returns JSON, we parse it directly.
        let response: TmdbSearchResponse = serde_json::from_str(&html)?;

        if let Some(anime) = response.results.first() {
            let poster = anime.poster_path.as_ref().map(|p| format!("{}{}", Self::IMAGE_BASE, p));
            let backdrop = anime.backdrop_path.as_ref().map(|p| format!("{}{}", Self::IMAGE_BASE, p));
            return Ok((poster, backdrop));
        }

        Ok((None, None))
    }
}
