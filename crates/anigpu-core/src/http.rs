use std::sync::OnceLock;
use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderName, HeaderValue, ACCEPT, ACCEPT_LANGUAGE, REFERER, USER_AGENT};

use crate::Result;

pub const DEFAULT_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/114.0.0.0 Safari/537.36";

static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

pub fn client() -> &'static reqwest::Client {
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("failed to build http client")
    })
}

/// Build a header map from `(name, value)` pairs; bad pairs are skipped.
fn build_headers(headers: &[(&str, &str)]) -> HeaderMap {
    let mut map = HeaderMap::new();
    map.insert(USER_AGENT, HeaderValue::from_static(DEFAULT_UA));
    map.insert(ACCEPT, HeaderValue::from_static(
        "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8",
    ));
    map.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("es-ES,es;q=0.9,en;q=0.8"));
    for (name, value) in headers {
        if let (Ok(n), Ok(v)) = (
            HeaderName::from_bytes(name.as_bytes()),
            HeaderValue::from_str(value),
        ) {
            map.insert(n, v);
        }
    }
    map
}

pub async fn get_text(url: &str) -> Result<String> {
    get_text_with(url, &[]).await
}

pub async fn get_text_with(url: &str, headers: &[(&str, &str)]) -> Result<String> {
    let resp = client()
        .get(url)
        .headers(build_headers(headers))
        .send()
        .await?
        .error_for_status()?;
    Ok(resp.text().await?)
}

pub async fn post_form(url: &str, form: &[(&str, &str)]) -> Result<String> {
    let resp = client()
        .post(url)
        .headers(build_headers(&[("Content-Type", "application/x-www-form-urlencoded")]))
        .form(form)
        .send()
        .await?
        .error_for_status()?;
    Ok(resp.text().await?)
}

/// Headers used by animeonlineninja (Chrome UA + referer).
pub fn ninja_headers() -> Vec<(&'static str, &'static str)> {
    vec![(REFERER.as_str(), "https://ww3.animeonline.ninja")]
}
