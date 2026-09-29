use scraper::{ElementRef, Html, Selector};

use crate::error::Error;
use crate::Result;

/// Parse a CSS selector once, mapping failures to our error type.
pub fn sel(s: &str) -> Result<Selector> {
    Selector::parse(s).map_err(|e| Error::Selector(format!("{s}: {e}")))
}

/// First match under `root` (descendants), trimmed text.
pub fn find_text<'a>(root: &ElementRef<'a>, selector: &Selector) -> String {
    root.select(selector)
        .next()
        .map(|e| e.text().collect::<String>().trim().to_string())
        .unwrap_or_default()
}

/// First match under a whole document, trimmed text.
pub fn doc_text<'a>(doc: &'a Html, selector: &Selector) -> String {
    doc.select(selector)
        .next()
        .map(|e| e.text().collect::<String>().trim().to_string())
        .unwrap_or_default()
}

/// Text of the first match, falling back to `fallback`'s first match.
pub fn find_text_or<'a>(root: &ElementRef<'a>, a: &Selector, b: &Selector) -> String {
    let first = find_text(root, a);
    if first.is_empty() {
        find_text(root, b)
    } else {
        first
    }
}

pub fn find_attr<'a>(root: &ElementRef<'a>, selector: &Selector, attr: &str) -> Option<String> {
    root.select(selector)
        .next()
        .and_then(|e| e.value().attr(attr))
        .map(|s| s.to_string())
}

pub fn doc_attr<'a>(doc: &'a Html, selector: &Selector, attr: &str) -> Option<String> {
    doc.select(selector)
        .next()
        .and_then(|e| e.value().attr(attr))
        .map(|s| s.to_string())
}

/// All matches under `root`.
pub fn select_all<'a>(root: &'a ElementRef<'a>, selector: &'a Selector) -> impl Iterator<Item = ElementRef<'a>> {
    root.select(selector)
}

/// `n` element-siblings after `node` (jQuery `.next().next()` semantics:
/// text nodes are ignored, `<br>` counts as one hop).
pub fn next_element_sibling<'a>(node: ElementRef<'a>) -> Option<ElementRef<'a>> {
    let mut cursor = node.next_sibling();
    while let Some(sibling) = cursor {
        if let Some(el) = ElementRef::wrap(sibling) {
            return Some(el);
        }
        cursor = sibling.next_sibling();
    }
    None
}

/// Direct element children of `root`.
pub fn children<'a>(root: &ElementRef<'a>) -> Vec<ElementRef<'a>> {
    root.children().filter_map(ElementRef::wrap).collect()
}

/// Direct element children of the first match of `selector` in `doc`.
pub fn doc_children<'a>(doc: &'a Html, selector: &Selector) -> Vec<ElementRef<'a>> {
    doc.select(selector).next().map(|el| children(&el)).unwrap_or_default()
}

/// `$(...).find(sel)` scoped to a document: first match of `container`,
/// then all descendants matching `selector`.
pub fn doc_find<'a>(doc: &'a Html, container: &Selector, selector: &'a Selector) -> Vec<ElementRef<'a>> {
    match doc.select(container).next() {
        Some(el) => el.select(selector).collect(),
        None => Vec::new(),
    }
}

/// Read an embedded JS JSON literal: `var name = {...};` / `[...]`.
pub fn extract_js_json(html: &str, var_name: &str) -> Option<serde_json::Value> {
    let start = html.find(&format!("var {var_name}"))?;
    let open_idx = html[start..]
        .find(['{', '['])
        .map(|i| start + i)?;
    let close = if html.as_bytes()[open_idx] == b'{' { '}' } else { ']' };
    let end = html[open_idx..].find(close)? + open_idx;
    serde_json::from_str(&html[open_idx..=end]).ok()
}
