use async_trait::async_trait;
use regex::Regex;
use scraper::Html;
use std::sync::OnceLock;

use super::Source;
use crate::http::get_text;
use crate::models::{
    AnimeDetails, CardItem, EpisodeItem, EpisodeNum, LatestItem, RelatedItem, ServerItem,
};
use crate::util::{doc_attr, doc_text, find_text, sel};
use crate::Result;

pub const BASE_URL: &str = "https://www4.animeflv.net";

pub struct AnimeFLV;

pub static ANIMEFLV: AnimeFLV = AnimeFLV;

fn episodes_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"var episodes = (\[.*?\]);").expect("valid regex"))
}

fn anime_info_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"var anime_info = (\[.*?\]);").expect("valid regex"))
}

fn videos_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"var videos = (\{.*?\});").expect("valid regex"))
}

fn slug_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(.+)-\d+$").expect("valid regex"))
}

fn prefix_url(path: &str) -> String {
    if path.starts_with("http") {
        path.to_string()
    } else if path.starts_with('/') {
        format!("{BASE_URL}{path}")
    } else {
        path.to_string()
    }
}

/// Parse the common `.ListAnimes li article` grid used by search and browse.
fn parse_list_animes(doc: &Html) -> crate::Result<Vec<CardItem>> {
    let item_sel = sel(".ListAnimes li article")?;
    let h3_sel = sel("h3.Title")?;
    let a_sel = sel("a")?;
    let img_sel = sel("img")?;

    let mut results = Vec::new();
    for article in doc.select(&item_sel) {
        let title = find_text(&article, &h3_sel);
        let url_path = article
            .select(&a_sel)
            .next()
            .and_then(|a| a.value().attr("href"))
            .unwrap_or_default();
        let image = article
            .select(&img_sel)
            .next()
            .and_then(|img| img.value().attr("src"))
            .map(|s| prefix_url(s));

        results.push(CardItem {
            title,
            url: format!("{BASE_URL}{url_path}"),
            image,
            anime_url: None,
        });
    }
    Ok(results)
}

#[async_trait]
impl Source for AnimeFLV {
    fn id(&self) -> &'static str {
        "animeflv"
    }

    fn name(&self) -> &'static str {
        "AnimeFLV"
    }

    async fn get_latest(&self) -> Result<Vec<LatestItem>> {
        let html = get_text(BASE_URL).await?;
        let doc = Html::parse_document(&html);

        let li_a_sel = sel(".ListEpisodios li a")?;
        let title_sel = sel(".Title")?;
        let capi_sel = sel(".Capi")?;
        let img_sel = sel("img")?;

        let mut results = Vec::new();
        for a in doc.select(&li_a_sel) {
            let url_path = a.value().attr("href").unwrap_or_default().to_string();
            let title = find_text(&a, &title_sel);
            let episode = find_text(&a, &capi_sel);
            let image = a
                .select(&img_sel)
                .next()
                .and_then(|img| img.value().attr("src"))
                .map(|s| prefix_url(s));

            let mut cover = image.clone();
            let mut anime_url = format!("{BASE_URL}{url_path}");

            if url_path.contains("/ver/") {
                let ver_slug = url_path.replace("/ver/", "");
                if let Some(caps) = slug_re().captures(&ver_slug) {
                    let anime_slug = &caps[1];
                    cover = Some(format!(
                        "{BASE_URL}/uploads/animes/covers/{anime_slug}.jpg"
                    ));
                    anime_url = format!("{BASE_URL}/anime/{anime_slug}");
                }
            }

            results.push(LatestItem {
                title,
                episode: Some(episode),
                image,
                cover,
                anime_url: Some(anime_url),
                url: format!("{BASE_URL}{url_path}"),
            });
        }
        Ok(results)
    }

    async fn get_details(&self, url: &str) -> Result<AnimeDetails> {
        // If it's an episode URL (/ver/...), resolve to the anime page first.
        let anime_url = if url.contains("/ver/") {
            let ep_html = get_text(url).await?;
            let ep_doc = Html::parse_document(&ep_html);
            let nav_sel = sel(".CapNvLs")?;
            doc_attr(&ep_doc, &nav_sel, "href")
                .map(|p| format!("{BASE_URL}{p}"))
                .unwrap_or_else(|| url.to_string())
        } else {
            url.to_string()
        };

        let html = get_text(&anime_url).await?;
        let doc = Html::parse_document(&html);

        let h1_sel = sel("h1.Title")?;
        let desc_sel = sel(".Description p")?;
        let cover_sel = sel(".AnimeCover .Image figure img")?;
        let status_sel = sel(".AnmStts span")?;
        let genre_sel = sel(".Genres a")?;
        let related_sel = sel(".ListAnmRel li")?;
        let a_sel = sel("a")?;

        let title = doc_text(&doc, &h1_sel);
        let synopsis = doc_text(&doc, &desc_sel);
        let cover = doc_attr(&doc, &cover_sel, "src").map(|s| prefix_url(&s));
        let status = doc_text(&doc, &status_sel);

        let mut genres = Vec::new();
        for el in doc.select(&genre_sel) {
            genres.push(el.text().collect::<String>().trim().to_string());
        }

        let mut related = Vec::new();
        for li in doc.select(&related_sel) {
            if let Some(link) = li.select(&a_sel).next() {
                let rel_title = link.text().collect::<String>().trim().to_string();
                let rel_url = link.value().attr("href").unwrap_or_default().to_string();
                let full_text = li.text().collect::<String>();
                let kind = full_text
                    .replace(&rel_title, "")
                    .trim()
                    .to_string();
                if !rel_url.is_empty() {
                    related.push(RelatedItem {
                        title: rel_title,
                        url: prefix_url(&rel_url),
                        image: None,
                        kind: Some(if kind.is_empty() {
                            "Relacionado".into()
                        } else {
                            kind
                        }),
                    });
                }
            }
        }

        // Episodes from embedded script: var episodes = [[1,...],...]; var anime_info = [id,...,slug,...];
        let mut episodes = Vec::new();
        if let Some(ep_caps) = episodes_re().captures(&html) {
            if let Some(info_caps) = anime_info_re().captures(&html) {
                if let (Ok(ep_data), Ok(info_data)) = (
                    serde_json::from_str::<serde_json::Value>(&ep_caps[1]),
                    serde_json::from_str::<serde_json::Value>(&info_caps[1]),
                ) {
                    let _anime_id = info_data
                        .as_array()
                        .and_then(|a| a.first())
                        .and_then(|v| v.as_str().or_else(|| v.as_i64().map(|_| "")))
                        .unwrap_or_default();
                    let anime_id_num = info_data
                        .as_array()
                        .and_then(|a| a.first())
                        .and_then(|v| {
                            v.as_i64()
                                .or_else(|| v.as_str().and_then(|s| s.parse::<i64>().ok()))
                        });
                    let anime_slug = info_data
                        .as_array()
                        .and_then(|a| a.get(2))
                        .and_then(|v| v.as_str())
                        .unwrap_or_default();

                    if let Some(arr) = ep_data.as_array() {
                        for ep in arr {
                            if let Some(ep_arr) = ep.as_array() {
                                let ep_num = ep_arr
                                    .first()
                                    .and_then(|v| {
                                        v.as_i64()
                                            .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
                                    })
                                    .unwrap_or(0) as u32;

                                let image = anime_id_num.map(|id| {
                                    format!(
                                        "https://cdn.animeflv.net/screenshots/{id}/{ep_num}/th_3.jpg"
                                    )
                                });

                                episodes.push(EpisodeItem {
                                    episode: EpisodeNum::Int(ep_num),
                                    url: format!("{BASE_URL}/ver/{anime_slug}-{ep_num}"),
                                    image,
                                });
                            }
                        }
                    }
                }
            }
        }

        Ok(AnimeDetails {
            title,
            synopsis,
            cover,
            backdrop: None,
            status: if status.is_empty() { None } else { Some(status) },
            genres,
            related,
            episodes,
        })
    }

    async fn get_servers(&self, url: &str) -> Result<Vec<ServerItem>> {
        let html = get_text(url).await?;

        if let Some(caps) = videos_re().captures(&html) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&caps[1]) {
                if let Some(sub_arr) = val.get("SUB").and_then(|v| v.as_array()) {
                    let mut servers = Vec::new();
                    for item in sub_arr {
                        let title = item
                            .get("title")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Unknown")
                            .to_string();
                        let code = item
                            .get("code")
                            .or_else(|| item.get("url"))
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        let server_name = item
                            .get("server")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        servers.push(ServerItem {
                            server: server_name,
                            title,
                            code,
                            url: None,
                        });
                    }
                    return Ok(servers);
                }
            }
        }
        Ok(Vec::new())
    }

    async fn search(&self, query: &str) -> Result<Vec<CardItem>> {
        let url = format!("{BASE_URL}/browse?q={}", urlencoding::encode(query));
        let html = get_text(&url).await?;
        let doc = Html::parse_document(&html);
        parse_list_animes(&doc)
    }

    async fn browse(&self, page: u32) -> Result<Vec<CardItem>> {
        let url = format!("{BASE_URL}/browse?page={page}");
        let html = get_text(&url).await?;
        let doc = Html::parse_document(&html);
        parse_list_animes(&doc)
    }
}
