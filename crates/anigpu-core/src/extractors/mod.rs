use regex::Regex;
use std::sync::OnceLock;

use crate::models::Subtitle;

macro_rules! lazy_re {
    ($name:ident, $pattern:expr) => {
        fn $name() -> &'static Regex {
            static CELL: OnceLock<Regex> = OnceLock::new();
            CELL.get_or_init(|| Regex::new($pattern).expect("valid regex"))
        }
    };
}

lazy_re!(m3u8_re, r#"(https?://[^\s"'<>]+\.m3u8[^\s"'<>]*)"#);
lazy_re!(m3u8_re_i, r#"(?i)(https?://[^\s"'<>]+\.m3u8[^\s"'<>]*)"#);
lazy_re!(mp4_re, r#"(https?://[^\s"'<>]+\.mp4[^\s"'<>]*)"#);
lazy_re!(jw_sources_re, r"sources:\s*(\[[^\]]+\])");
lazy_re!(jw_file_re, r#"file\s*:\s*["']([^"']+)["']"#);
lazy_re!(jw_tracks_re, r"tracks:\s*(\[[^\]]+\])");
lazy_re!(jw_label_re, r#"label\s*:\s*["']([^"']+)["']"#);
lazy_re!(jw_kind_re, r#"kind\s*:\s*["']([^"']+)["']"#);
lazy_re!(packed_re, r"eval\(function\(p,a,c,k,e,?[d]?\).*?\.split\('\|'\).*?\)");

/// First `.m3u8` URL found in `text`.
pub fn extract_m3u8_from_text(text: &str) -> Option<String> {
    if let Some(caps) = m3u8_re().captures(text) {
        return Some(caps[1].to_string());
    }
    m3u8_re_i().find(text).map(|m| m.as_str().to_string())
}

/// First `.mp4` URL found in `text`.
pub fn extract_mp4_from_text(text: &str) -> Option<String> {
    mp4_re().captures(text).map(|c| c[1].to_string())
}

#[derive(Debug, Default, Clone)]
pub struct JwPlayerConfig {
    pub sources: Vec<String>,
    pub tracks: Vec<Subtitle>,
}

/// Pull `sources`/`tracks` out of an inline JWPlayer config object.
pub fn parse_jw_player(html: &str) -> JwPlayerConfig {
    let mut config = JwPlayerConfig::default();

    if let Some(caps) = jw_sources_re().captures(html) {
        for f in jw_file_re().captures_iter(&caps[1]) {
            config.sources.push(f[1].to_string());
        }
    }

    if let Some(caps) = jw_tracks_re().captures(html) {
        for block in caps[1].split('}').filter(|b| b.contains("file")) {
            let Some(file) = jw_file_re().captures(block) else {
                continue;
            };
            let kind_val = jw_kind_re().captures(block).map(|k| k[1].to_string());
            match kind_val.as_deref() {
                None | Some("captions") | Some("subtitles") => {}
                _ => continue,
            }
            let label = jw_label_re()
                .captures(block)
                .map(|l| l[1].to_string())
                .unwrap_or_else(|| "Subtítulos".to_string());
            config.tracks.push(Subtitle {
                file: file[1].to_string(),
                label,
            });
        }
    }

    config
}

/// Pack obfuscated scripts (`eval(function(p,a,c,k,e,d){...})`) found in the page.
pub fn extract_packed(html: &str) -> Vec<String> {
    packed_re()
        .find_iter(html)
        .map(|m| m.as_str().to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn m3u8() {
        let text = r#"x = "https://cdn.example.com/hls/master.m3u8?token=1";"#;
        assert_eq!(
            extract_m3u8_from_text(text).as_deref(),
            Some("https://cdn.example.com/hls/master.m3u8?token=1")
        );
    }

    #[test]
    fn mp4() {
        assert_eq!(
            extract_mp4_from_text(r#"src: "https://a.b/c.mp4""#).as_deref(),
            Some("https://a.b/c.mp4")
        );
    }

    #[test]
    fn jwplayer() {
        let html = r#"
            sources: [{file: "https://cdn/x/master.m3u8", label: "auto"}],
            tracks: [{file: "https://cdn/subs.vtt", label: "Español", kind: "captions"},
                     {file: "https://cdn/prev.vtt", kind: "thumbnails"}]
        "#;
        let cfg = parse_jw_player(html);
        assert_eq!(cfg.sources, vec!["https://cdn/x/master.m3u8"]);
        assert_eq!(cfg.tracks.len(), 1);
        assert_eq!(cfg.tracks[0].label, "Español");
    }
}
