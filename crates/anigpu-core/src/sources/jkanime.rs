use async_trait::async_trait;
use regex::Regex;
use scraper::Html;
use std::collections::HashSet;
use std::sync::OnceLock;

use super::Source;
use crate::http::get_text;
use crate::models::{
    AnimeDetails, CardItem, EpisodeItem, EpisodeNum, LatestItem, RelatedItem, ServerItem,
};
use crate::util::{doc_attr, doc_text, find_text, sel};
use crate::Result;

pub const BASE_URL: &str = "https://jkanime.net";

pub struct JkAnime;

pub static JKANIME: JkAnime = JkAnime;

fn video_iframe_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"video\[\d+\]\s*=\s*'[^']*src="([^"]+)""#).expect("valid regex"))
}

fn animes_json_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"var animes = (\{.*?\});").expect("valid regex"))
}

fn ep_num_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"/(\d+)/$").expect("valid regex"))
}

/// Parse the embedded `var animes = {...};` JSON and return cards.
fn parse_animes_json(html: &str) -> Vec<CardItem> {
    let Some(caps) = animes_json_re().captures(html) else {
        return Vec::new();
    };
    let json_str = &caps[1];
    let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str) else {
        return Vec::new();
    };
    let Some(data) = val.get("data").and_then(|d| d.as_array()) else {
        return Vec::new();
    };
    data.iter()
        .filter_map(|a| {
            let title = a.get("title")?.as_str()?.to_string();
            let url = a.get("url")?.as_str()?.to_string();
            let image = a.get("image").and_then(|i| i.as_str()).map(|s| s.to_string());
            Some(CardItem {
                title,
                url: url.clone(),
                image,
                anime_url: Some(url),
            })
        })
        .collect()
}

#[async_trait]
impl Source for JkAnime {
    fn id(&self) -> &'static str {
        "jkanime"
    }

    fn name(&self) -> &'static str {
        "JKAnime"
    }

    async fn get_latest(&self) -> Result<Vec<LatestItem>> {
        let html = get_text(BASE_URL).await?;
        let doc = Html::parse_document(&html);

        let card_sel = sel(".card.ml-2.mr-2")?;
        let a_sel = sel("a")?;
        let img_sel = sel("img")?;
        let h5_sel = sel("h5")?;
        let badge_sel = sel(".badge-primary")?;

        let anime_url_re = Regex::new(r"(https://jkanime\.net/[^/]+/)\d+/$").unwrap();

        let mut results = Vec::new();
        for card in doc.select(&card_sel) {
            let Some(a) = card.select(&a_sel).next() else {
                continue;
            };
            let url_path = a.value().attr("href").unwrap_or_default().to_string();

            let image = a
                .select(&img_sel)
                .next()
                .and_then(|img| {
                    img.value()
                        .attr("data-animepic")
                        .or_else(|| img.value().attr("src"))
                })
                .map(|s| s.to_string());

            let title = find_text(&a, &h5_sel);
            let episode_text = find_text(&a, &badge_sel);
            let episode = episode_text.replace("Ep ", "");

            let cover = image.clone();
            let anime_url = if let Some(caps) = anime_url_re.captures(&url_path) {
                caps[1].to_string()
            } else {
                url_path.clone()
            };

            results.push(LatestItem {
                title,
                episode: Some(episode),
                image,
                cover,
                anime_url: Some(anime_url),
                url: url_path,
            });
        }
        Ok(results)
    }

    async fn get_details(&self, url: &str) -> Result<AnimeDetails> {
        let html = get_text(url).await?;
        let doc = Html::parse_document(&html);

        // Title
        let info_h3_sel = sel(".anime_info h3")?;
        let title_sel = sel("title")?;
        let mut title = doc_text(&doc, &info_h3_sel);
        if title.is_empty() {
            title = doc_text(&doc, &title_sel);
            // Clean up page title
            title = Regex::new(r" - anime .* online JkAnime$")
                .unwrap()
                .replace(&title, "")
                .replace(" - JkAnime", "")
                .trim()
                .to_string();
        }

        // Synopsis
        let synopsis_sel = sel(".anime_info p.scroll")?;
        let synopsis_alt_sel = sel("p[rel=\"sinopsis\"]")?;
        let mut synopsis = doc_text(&doc, &synopsis_sel);
        if synopsis.is_empty() {
            synopsis = doc_text(&doc, &synopsis_alt_sel);
        }

        // Cover
        let og_image_sel = sel("meta[property=\"og:image\"]")?;
        let img_image_sel = sel("img[src*=\"/image/\"]")?;
        let cover = doc_attr(&doc, &og_image_sel, "content")
            .or_else(|| doc_attr(&doc, &img_image_sel, "src"));

        // Status
        let anime_data_sel = sel(".anime_data")?;
        let enemision_sel = sel(".enemision")?;
        let status = doc
            .select(&anime_data_sel)
            .next()
            .and_then(|block| {
                block
                    .select(&enemision_sel)
                    .next()
                    .map(|e| e.text().collect::<String>().trim().to_string())
            })
            .unwrap_or_else(|| "En emisión".to_string());

        // Genres (deduplicate)
        let genre_sel = sel("a[href*=\"/genero/\"]")?;
        let mut genre_set = HashSet::new();
        for el in doc.select(&genre_sel) {
            let text = el.text().collect::<String>().trim().to_string();
            if !text.is_empty() {
                genre_set.insert(text);
            }
        }
        let genres: Vec<String> = genre_set.into_iter().collect();

        // Related
        let aditional_sel = sel("#aditional")?;
        let mut related = Vec::new();
        for el in doc.select(&aditional_sel) {
            let kind = el.text().collect::<String>().trim().to_string();
            // Walk next element siblings (skip <br>)
            let mut cursor = crate::util::next_element_sibling(el);
            while let Some(next_el) = cursor {
                if next_el.value().name() == "a" {
                    let rel_title = next_el.text().collect::<String>().trim().to_string();
                    let rel_url = next_el
                        .value()
                        .attr("href")
                        .unwrap_or_default()
                        .to_string();
                    if !rel_url.is_empty() {
                        related.push(RelatedItem {
                            title: rel_title,
                            url: rel_url,
                            image: None,
                            kind: Some(kind.clone()),
                        });
                    }
                    // Skip the <br> after <a>, then get the next element
                    cursor = crate::util::next_element_sibling(next_el)
                        .and_then(|br| crate::util::next_element_sibling(br));
                } else {
                    break;
                }
            }
        }

        // Total episodes
        let li_sel = sel("li")?;
        let uep_sel = sel("#uep")?;
        let mut total_episodes: u32 = 0;

        if let Some(first_data) = doc.select(&anime_data_sel).next() {
            for li in first_data.select(&li_sel) {
                let text = li.text().collect::<String>();
                if text.contains("Episodios:") {
                    if let Some(caps) = Regex::new(r"\d+").unwrap().find(&text) {
                        total_episodes = caps.as_str().parse().unwrap_or(0);
                    }
                    break;
                }
            }
        }

        if total_episodes == 0 {
            if let Some(uep_href) = doc_attr(&doc, &uep_sel, "href") {
                if let Some(caps) = ep_num_re().captures(&uep_href) {
                    total_episodes = caps[1].parse().unwrap_or(0);
                }
            }
        }

        let mut episodes = Vec::new();
        if total_episodes > 0 {
            let base = if url.ends_with('/') {
                url.to_string()
            } else {
                format!("{url}/")
            };
            for i in (1..=total_episodes).rev() {
                episodes.push(EpisodeItem {
                    episode: EpisodeNum::Int(i),
                    url: format!("{base}{i}/"),
                    image: cover.clone(),
                });
            }
        }

        Ok(AnimeDetails {
            title,
            synopsis,
            cover,
            backdrop: None,
            status: Some(status),
            genres,
            related,
            episodes,
        })
    }

    async fn get_servers(&self, url: &str) -> Result<Vec<ServerItem>> {
        let html = get_text(url).await?;
        let doc = Html::parse_document(&html);

        // Extract iframe URLs from video[N] = '...'
        let server_urls: Vec<String> = video_iframe_re()
            .captures_iter(&html)
            .map(|c| c[1].to_string())
            .collect();

        // Extract server names from tabs
        let option_sel = sel("a[href^=\"#option\"]")?;
        let server_names: Vec<String> = doc
            .select(&option_sel)
            .map(|el| el.text().collect::<String>().trim().to_string())
            .collect();

        let mut servers = Vec::new();
        for i in 0..server_urls.len().min(server_names.len()) {
            servers.push(ServerItem {
                server: Some(server_names[i].clone()),
                title: server_names[i].clone(),
                code: None,
                url: Some(server_urls[i].clone()),
            });
        }
        Ok(servers)
    }

    async fn search(&self, query: &str) -> Result<Vec<CardItem>> {
        let url = format!("{BASE_URL}/buscar/{}/", urlencoding::encode(query));
        let html = get_text(&url).await?;
        let doc = Html::parse_document(&html);

        let item_sel = sel(".anime__item")?;
        let a_sel = sel("a")?;
        let pic_sel = sel(".anime__item__pic")?;
        let img_sel = sel("img")?;
        let h5a_sel = sel("h5 a")?;
        let h5_sel = sel(".anime__item__text h5")?;

        let mut results = Vec::new();
        for item in doc.select(&item_sel) {
            let href = item
                .select(&a_sel)
                .next()
                .and_then(|a| a.value().attr("href"))
                .unwrap_or_default()
                .to_string();

            let image = item
                .select(&pic_sel)
                .next()
                .and_then(|el| el.value().attr("data-setbg"))
                .or_else(|| {
                    item.select(&img_sel)
                        .next()
                        .and_then(|img| img.value().attr("src"))
                })
                .map(|s| s.to_string());

            let mut title = find_text(&item, &h5a_sel);
            if title.is_empty() {
                title = find_text(&item, &h5_sel);
            }

            if !href.is_empty() {
                results.push(CardItem {
                    title,
                    url: href,
                    image,
                    anime_url: None,
                });
            }
        }

        // Fallback: try embedded JSON
        if results.is_empty() {
            results = parse_animes_json(&html);
        }

        Ok(results)
    }

    async fn browse(&self, page: u32) -> Result<Vec<CardItem>> {
        let url = if page <= 1 {
            format!("{BASE_URL}/directorio/1/")
        } else {
            format!("{BASE_URL}/directorio/1?p={page}")
        };
        let html = get_text(&url).await?;
        Ok(parse_animes_json(&html))
    }
}
