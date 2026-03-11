pub mod html;
pub mod terminal;

/// Split a snapshot ID into (group, page_key).
///
/// "foo/bar/page_1" → ("foo/bar", "page_1")
/// "foo/bar/page"   → ("foo/bar", "page")
/// "foo/bar"        → ("foo/bar", "")
pub fn split_page_key(id: &str) -> (&str, &str) {
    if let Some(pos) = id.rfind("/page") {
        (&id[..pos], &id[pos + 1..])
    } else {
        (id, "")
    }
}

/// Pretty-print a page key: "page_1" → "page 1", "page" → "page".
pub fn display_page_key(key: &str) -> String {
    if let Some(n) = key.strip_prefix("page_") {
        format!("page {n}")
    } else {
        key.to_string()
    }
}
