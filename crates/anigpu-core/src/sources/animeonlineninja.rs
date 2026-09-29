use async_trait::async_trait;
use regex::Regex;
use scraper::Html;

use super::Source;
use crate::http::{get_text_with, ninja_headers, post_form};
use crate::models::{AnimeDetails, CardItem, EpisodeItem, EpisodeNum, LatestItem, ServerItem};
use crate::util::{doc_text, find_text, sel};
use crate::Result;

pub const BASE_URL: &str = "https://ww3.animeonline.ninja";

pub struct AnimeOnlineNinja;

pub static ANIMEONLINENINJA: AnimeOnlineNinja = AnimeOnlineNinja;

fn nh() -> Vec<(&'static str, &'static str)> {
    ninja_headers()
}

/// Shared parser for search / browse article grids.
fn parse_item_articles(doc: &Html) -> crate::Result<Vec<CardItem>> {
    let item_sel = sel("article.item")?;
    let a_sel = sel("a")?;
    let img_sel = sel("img")?;
    let h3_sel = sel(".data h3")?;
    let h4_sel = sel(".data h4")?;

    let mut results = Vec::new();
    for article in doc.select(&item_sel) {
        let url = article
            .select(&a_sel)
            .next()
            .and_then(|a| a.value().attr("href"))
            .unwrap_or_default()
            .to_string();

        let image = article
            .select(&img_sel)
            .next()
            .and_then(|img| {
                img.value()
                    .attr("data-src")
                    .or_else(|| img.value().attr("src"))
            })
            .map(|s| s.to_string());

        let mut title = find_text(&article, &h3_sel);
        if title.is_empty() {
            title = find_text(&article, &h4_sel);
        }

        if !url.is_empty() {
            results.push(CardItem {
                title,
                image,
                url,
                anime_url: None,
            });
        }
    }
    Ok(results)
}

#[async_trait]
impl Source for AnimeOnlineNinja {
    fn id(&self) -> &'static str {
        "animeonlineninja"
    }

    fn name(&self) -> &'static str {
        "AnimeOnline Ninja"
    }

    async fn get_latest(&self) -> Result<Vec<LatestItem>> {
        let headers = nh();
        let header_refs: Vec<(&str, &str)> = headers.iter().map(|(k, v)| (*k, *v)).collect();
        let html = get_text_with(&format!("{BASE_URL}/inicio/"), &header_refs).await?;
        let doc = Html::parse_document(&html);

        let ep_sel = sel("article.episodes")?;
        let a_sel = sel("a")?;
        let img_sel = sel("img")?;
        let title_sel = sel(".data h3")?;
        let ep_num_sel = sel(".epiposter h4")?;

        let mut results = Vec::new();
        for article in doc.select(&ep_sel) {
            let url = article
                .select(&a_sel)
                .next()
                .and_then(|a| a.value().attr("href"))
                .unwrap_or_default()
                .to_string();

            let image = article
                .select(&img_sel)
                .next()
                .and_then(|img| {
                    img.value()
                        .attr("data-src")
                        .or_else(|| img.value().attr("src"))
                })
                .map(|s| s.to_string());

            let title = find_text(&article, &title_sel);
            let mut ep_text = find_text(&article, &ep_num_sel);
            ep_text = ep_text.replace("Episodio", "").trim().to_string();

            if !url.is_empty() {
                results.push(LatestItem {
                    title,
                    episode: if ep_text.is_empty() {
                        None
                    } else {
                        Some(format!("Episodio {ep_text}"))
                    },
                    image,
                    cover: None,
                    anime_url: Some(url.clone()),
                    url,
                });
            }
        }
        Ok(results)
    }

    async fn get_details(&self, url: &str) -> Result<AnimeDetails> {
        let headers = nh();
        let header_refs: Vec<(&str, &str)> = headers.iter().map(|(k, v)| (*k, *v)).collect();
        let html = get_text_with(url, &header_refs).await?;
        let doc = Html::parse_document(&html);

        let title_sel = sel(".sheader .data h1")?;
        let title_alt_sel = sel("h1")?;
        let synopsis_sel = sel(".wp-content p")?;
        let poster_sel = sel(".sheader .poster img")?;
        let status_sel = sel(".status .text")?;
        let genre_sel = sel(".sgeneros a")?;

        let mut title = doc_text(&doc, &title_sel);
        if title.is_empty() {
            title = doc_text(&doc, &title_alt_sel);
        }

        let synopsis = doc_text(&doc, &synopsis_sel);

        let cover = doc
            .select(&poster_sel)
            .next()
            .and_then(|img| {
                img.value()
                    .attr("data-src")
                    .or_else(|| img.value().attr("src"))
            })
            .map(|s| s.to_string());

        let status = doc_text(&doc, &status_sel);

        let mut genres = Vec::new();
        for el in doc.select(&genre_sel) {
            genres.push(el.text().collect::<String>().trim().to_string());
        }

        // Episodes from #seasons .se-c .episodios li
        let ep_li_sel = sel("#seasons .se-c .episodios li")?;
        let numbering_sel = sel(".numerando")?;
        let ep_title_sel = sel(".episodiotitle a")?;
        let ep_img_sel = sel(".imagen img")?;

        let mut episodes = Vec::new();
        for li in doc.select(&ep_li_sel) {
            let mut ep_num = find_text(&li, &numbering_sel);
            // Format is often "1 - 24" (season - episode); take the episode part
            if ep_num.contains('-') {
                ep_num = ep_num
                    .split('-')
                    .nth(1)
                    .unwrap_or(&ep_num)
                    .trim()
                    .to_string();
            }

            let ep_url = li
                .select(&ep_title_sel)
                .next()
                .and_then(|a| a.value().attr("href"))
                .unwrap_or_default()
                .to_string();

            let ep_image = li
                .select(&ep_img_sel)
                .next()
                .and_then(|img| {
                    img.value()
                        .attr("data-src")
                        .or_else(|| img.value().attr("src"))
                })
                .map(|s| s.to_string());

            if !ep_url.is_empty() {
                episodes.push(EpisodeItem {
                    episode: EpisodeNum::Str(ep_num),
                    url: ep_url,
                    image: ep_image,
                });
            }
        }

        Ok(AnimeDetails {
            title,
            synopsis,
            cover,
            backdrop: None,
            status: if status.is_empty() {
                None
            } else {
                Some(status)
            },
            genres,
            related: Vec::new(),
            episodes,
        })
    }

    async fn get_servers(&self, url: &str) -> Result<Vec<ServerItem>> {
        let headers = nh();
        let header_refs: Vec<(&str, &str)> = headers.iter().map(|(k, v)| (*k, *v)).collect();
        let html = get_text_with(url, &header_refs).await?;
        struct PendingServer {
            title: String,
            data_type: String,
            data_post: String,
            data_nume: String,
        }

        // Scope the HTML document so it's dropped before the async POST loop
        // (Html is !Send because of non-atomic tendrils)
        let pending: Vec<PendingServer> = {
            let doc = Html::parse_document(&html);
            let li_sel = sel("#playeroptions ul li")?;
            let title_sel = sel(".title")?;

            let mut result = Vec::new();
            for li in doc.select(&li_sel) {
                let server_name = find_text(&li, &title_sel);
                let data_type = li.value().attr("data-type").unwrap_or_default().to_string();
                let data_post = li.value().attr("data-post").unwrap_or_default().to_string();
                let data_nume = li.value().attr("data-nume").unwrap_or_default().to_string();

                if !data_post.is_empty() && !data_nume.is_empty() {
                    result.push(PendingServer {
                        title: server_name,
                        data_type,
                        data_post,
                        data_nume,
                    });
                }
            }
            result
        };

        let iframe_src_re = Regex::new(r#"src="([^"]+)""#).unwrap();
        let mut servers = Vec::new();
        for ps in &pending {
            let form = [
                ("action", "doo_player_ajax"),
                ("post", &ps.data_post),
                ("nume", &ps.data_nume),
                ("type", &ps.data_type),
            ];
            match post_form(&format!("{BASE_URL}/wp-admin/admin-ajax.php"), &form).await {
                Ok(body) => {
                    // Response is JSON like {"embed_url":"<iframe src=\"...\" ...>"}
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&body) {
                        if let Some(embed_html) = val.get("embed_url").and_then(|v| v.as_str()) {
                            let code = if let Some(caps) = iframe_src_re.captures(embed_html) {
                                Some(caps[1].to_string())
                            } else {
                                Some(embed_html.to_string())
                            };
                            if code.is_some() {
                                servers.push(ServerItem {
                                    server: Some(ps.title.clone()),
                                    title: ps.title.clone(),
                                    code,
                                    url: None,
                                });
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "[AnimeOnlineNinja] Failed to resolve server {}: {}",
                        ps.title,
                        e
                    );
                }
            }
        }

        Ok(servers)
    }

    async fn search(&self, query: &str) -> Result<Vec<CardItem>> {
        let headers = nh();
        let header_refs: Vec<(&str, &str)> = headers.iter().map(|(k, v)| (*k, *v)).collect();
        let url = format!("{BASE_URL}/?s={}", urlencoding::encode(query));
        let html = get_text_with(&url, &header_refs).await?;
        let doc = Html::parse_document(&html);
        parse_item_articles(&doc)
    }

    async fn browse(&self, page: u32) -> Result<Vec<CardItem>> {
        let headers = nh();
        let header_refs: Vec<(&str, &str)> = headers.iter().map(|(k, v)| (*k, *v)).collect();
        let url = format!("{BASE_URL}/online/page/{page}/");
        let html = get_text_with(&url, &header_refs).await?;
        let doc = Html::parse_document(&html);
        parse_item_articles(&doc)
    }
}
