use async_trait::async_trait;
use scraper::{ElementRef, Html};

use super::Source;
use crate::http::get_text;
use crate::models::{
    AnimeDetails, CardItem, EpisodeItem, EpisodeNum, LatestItem, RelatedItem, ServerItem,
};
use crate::util::{children, doc_attr, doc_text, find_text, sel};
use crate::Result;

pub const BASE_URL: &str = "https://animeav1.com";

pub struct AnimeAv1;

pub static ANIMEAV1: AnimeAv1 = AnimeAv1;

fn parse_grid_cards<'a>(doc: &'a Html, grid: &scraper::Selector) -> Vec<ElementRef<'a>> {
    // The results live in the second grid on listing pages, first one on home.
    match doc.select(grid).next() {
        Some(el) => children(&el),
        None => Vec::new(),
    }
}

/// Shared card parsing for search/browse (both use the last `.grid.grid-cols-2`).
async fn fetch_cards(url: &str) -> Result<Vec<CardItem>> {
    let html = get_text(url).await?;
    let doc = Html::parse_document(&html);

    let grid_sel = sel(".grid.grid-cols-2")?;
    let link_sel = sel("a[href*=\"/media/\"]")?;
    let title_sel = sel("h3")?;
    let alt_title_sel = sel("header div, div.font-bold")?;
    let img_sel = sel("img")?;

    let grids: Vec<_> = doc.select(&grid_sel).collect();
    let grid = grids.last();

    let mut results = Vec::new();
    if let Some(grid) = grid {
        for card in children(grid) {
            let link = card.select(&link_sel).next();
            let Some(link) = link else { continue };
            let title = {
                let t = find_text(&card, &title_sel);
                if t.is_empty() {
                    find_text(&card, &alt_title_sel)
                } else {
                    t
                }
            };
            let image = card
                .select(&img_sel)
                .next()
                .and_then(|img| {
                    img.value().attr("src").or_else(|| img.value().attr("data-src"))
                })
                .map(|s| s.to_string());
            let href = link.value().attr("href").unwrap_or_default().to_string();
            let title = if title.is_empty() {
                link.text().collect::<String>().replace("Ver ", "").trim().to_string()
            } else {
                title
            };
            results.push(CardItem {
                title,
                url: format!("{BASE_URL}{href}"),
                image,
                anime_url: None,
            });
        }
    }
    Ok(results)
}

#[async_trait]
impl Source for AnimeAv1 {
    fn id(&self) -> &'static str {
        "animeav1"
    }

    fn name(&self) -> &'static str {
        "AnimeAV1"
    }

    async fn get_latest(&self) -> Result<Vec<LatestItem>> {
        let html = get_text(BASE_URL).await?;
        let doc = Html::parse_document(&html);

        let grid_sel = sel(".grid.grid-cols-2")?;
        let link_sel = sel("a[href*=\"/media/\"]")?;
        let header_div_sel = sel("header div")?;
        let bold_div_sel = sel("div.font-bold")?;
        let lead_sel = sel(".text-lead")?;
        let xs_sel = sel("div.text-xs")?;
        let img_sel = sel("img")?;
        let poster_sel = sel("img[src*=\"poster\"], img[src*=\"cover\"], img[src*=\"Poster\"]")?;

        let mut results = Vec::new();
        for card in parse_grid_cards(&doc, &grid_sel) {
            let link = card.select(&link_sel).next();
            let Some(link) = link else { continue };

            let mut title = find_text(&card, &header_div_sel);
            if title.is_empty() {
                title = find_text(&card, &bold_div_sel);
            }

            let mut episode = find_text(&card, &lead_sel);
            if episode.is_empty() {
                episode = find_text(&card, &xs_sel);
            }

            let image = card
                .select(&img_sel)
                .next()
                .and_then(|img| img.value().attr("src"))
                .map(|s| s.to_string());

            let mut cover = image.clone();
            let href = link.value().attr("href").unwrap_or_default().to_string();
            let parts: Vec<&str> = href.split('/').filter(|p| !p.is_empty()).collect();

            let mut anime_url = format!("{BASE_URL}{href}");
            if parts.len() == 3 {
                // /media/{slug}/{epNum} → anime page is /media/{slug}
                anime_url = format!("{BASE_URL}/media/{}", parts[1]);
                if let Some(poster) = card.select(&poster_sel).next() {
                    if let Some(src) = poster.value().attr("src") {
                        cover = Some(src.to_string());
                    }
                }
            }

            if title.is_empty() {
                title = link
                    .text()
                    .collect::<String>()
                    .replace("Ver ", "")
                    .trim()
                    .to_string();
            }

            let episode = if episode.is_empty() {
                String::new()
            } else {
                format!("Episodio {episode}")
            };

            results.push(LatestItem {
                title,
                episode: Some(episode),
                image,
                cover,
                anime_url: Some(anime_url),
                url: format!("{BASE_URL}{href}"),
            });
        }
        Ok(results)
    }

    async fn get_details(&self, url: &str) -> Result<AnimeDetails> {
        // Episode URLs (/media/slug/ep) resolve to the anime page (/media/slug).
        let parts: Vec<&str> = url.split('/').filter(|p| !p.is_empty()).collect();
        let anime_url = if parts.len() > 4 {
            format!("{BASE_URL}/media/{}", parts[parts.len() - 2])
        } else {
            url.to_string()
        };

        let html = get_text(&anime_url).await?;
        let doc = Html::parse_document(&html);

        let h1_sel = sel("h1")?;
        let synopsis_sel = sel(".text-subs.leading-relaxed")?;
        let p_sel = sel("p")?;
        let poster_sel = sel("img[alt*=\"Poster\"]")?;
        let backdrop_sel = sel("img[alt*=\"Backdrop\"]")?;
        let any_img_sel = sel("img")?;
        let status_sel = sel("header .flex.flex-wrap.items-center.gap-2.text-sm span:last-child")?;
        let genre_sel = sel("a[href*=\"/catalogo?genre=\"]")?;
        let related_container_sel = sel(".gradient-cut")?;
        let related_item_sel = sel(".group\\/item")?;
        let episode_link_sel = sel("a[href*=\"/media/\"]")?;

        let title = doc_text(&doc, &h1_sel);

        let mut synopsis = doc_text(&doc, &synopsis_sel);
        if synopsis.is_empty() {
            synopsis = doc.select(&p_sel).next()
                .map(|p| p.text().collect::<String>().trim().to_string())
                .unwrap_or_default();
        }

        let cover = doc_attr(&doc, &poster_sel, "src")
            .or_else(|| doc_attr(&doc, &poster_sel, "data-src"))
            .or_else(|| {
                doc.select(&any_img_sel)
                    .nth(2)
                    .and_then(|i| i.value().attr("src"))
                    .map(|s| s.to_string())
            });

        let backdrop = doc_attr(&doc, &backdrop_sel, "src")
            .or_else(|| doc_attr(&doc, &backdrop_sel, "data-src"));

        let status = doc_text(&doc, &status_sel);

        let mut genres = Vec::new();
        for el in doc.select(&genre_sel) {
            let text = el.text().collect::<String>().trim().to_string();
            if !text.is_empty() {
                genres.push(text);
            }
        }

        let mut related = Vec::new();
        if let Some(container) = doc.select(&related_container_sel).next() {
            for item in container.select(&related_item_sel) {
                let rel_title = item
                    .select(&sel("h3")?)
                    .next()
                    .map(|h| h.text().collect::<String>().trim().to_string())
                    .unwrap_or_default();
                let rel_url = item
                    .select(&sel("a")?)
                    .next()
                    .and_then(|a| a.value().attr("href"))
                    .map(|s| s.to_string());
                let rel_image = item
                    .select(&sel("img")?)
                    .next()
                    .and_then(|i| i.value().attr("src"))
                    .map(|s| s.to_string());
                let relation = item
                    .select(&sel("span, div")?)
                    .find(|e| e.text().collect::<String>().contains('('))
                    .map(|e| e.text().collect::<String>().trim().to_string());
                if let Some(rel_url) = rel_url {
                    related.push(RelatedItem {
                        title: if rel_title.is_empty() {
                            item.text()
                                .collect::<String>()
                                .split('(')
                                .next()
                                .unwrap_or_default()
                                .trim()
                                .to_string()
                        } else {
                            rel_title
                        },
                        url: format!("{BASE_URL}{rel_url}"),
                        image: rel_image,
                        kind: Some(relation.filter(|s| !s.is_empty()).unwrap_or_else(|| "Relacionado".into())),
                    });
                }
            }
        }

        let mut episodes: Vec<EpisodeItem> = Vec::new();
        for link in doc.select(&episode_link_sel) {
            let href = link.value().attr("href").unwrap_or_default().to_string();
            let parts: Vec<&str> = href.split('/').filter(|p| !p.is_empty()).collect();
            if parts.len() == 3 {
                let ep_num = parts[2].to_string();
                if !episodes.iter().any(|e| e.episode.to_string() == ep_num) {
                    episodes.push(EpisodeItem {
                        episode: EpisodeNum::Str(ep_num),
                        url: format!("{BASE_URL}{href}"),
                        image: None,
                    });
                }
            }
        }
        episodes.sort_by(|a, b| {
            let ai = a.episode.as_i64().unwrap_or(0);
            let bi = b.episode.as_i64().unwrap_or(0);
            bi.cmp(&ai)
        });

        Ok(AnimeDetails {
            title,
            synopsis,
            cover,
            backdrop,
            status: if status.is_empty() { None } else { Some(status) },
            genres,
            related,
            episodes,
        })
    }

    async fn get_servers(&self, url: &str) -> Result<Vec<ServerItem>> {
        let html = get_text(url).await?;

        // Pages embed `{server:"...",url:"..."}` literals in their player script.
        let re = regex::Regex::new(r#"\{server:"([^"]+)",url:"([^"]+)"\}"#).unwrap();
        let mut servers = Vec::new();
        let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for caps in re.captures_iter(&html) {
            let title = caps[1].to_string();
            let code = caps[2].replace('\\', "");
            *counts.entry(title.clone()).or_insert(0) += 1;
            let count = counts[&title];
            servers.push(ServerItem {
                server: Some(title.clone()),
                title: if count > 1 { format!("{title} {count}") } else { title },
                code: Some(code),
                url: None,
            });
        }
        Ok(servers)
    }

    async fn search(&self, query: &str) -> Result<Vec<CardItem>> {
        let url = format!("{BASE_URL}/catalogo?search={}", urlencoding::encode(query));
        fetch_cards(&url).await
    }

    async fn browse(&self, page: u32) -> Result<Vec<CardItem>> {
        let url = format!("{BASE_URL}/catalogo?page={page}");
        fetch_cards(&url).await
    }
}
