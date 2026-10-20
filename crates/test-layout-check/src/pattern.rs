//! Whether a path matches a pattern, the way a project states an exemption.
//!
//! A pattern is `/`-separated, `*` stays inside one path segment and `**`
//! crosses one. The same shape the comment policy uses, so one project states
//! its exemptions the same way for both rules.

/// Whether `path` matches `pattern`.
pub fn matches(pattern: &str, path: &str) -> bool {
    let pattern: Vec<&str> = pattern.split('/').filter(|part| !part.is_empty()).collect();
    let path: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    segments_match(&pattern, &path)
}

/// The pattern's segments against the path's, in order.
fn segments_match(pattern: &[&str], path: &[&str]) -> bool {
    let Some((first, rest)) = pattern.split_first() else {
        return path.is_empty();
    };
    if *first == "**" {
        // `**` crosses any number of segments, so every split is tried. The
        // longest first, so a pattern that ends in `**` matches at once.
        for skip in (0..=path.len()).rev() {
            if segments_match(rest, &path[skip..]) {
                return true;
            }
        }
        return false;
    }
    let Some((head, tail)) = path.split_first() else {
        return false;
    };
    segment_matches(first, head) && segments_match(rest, tail)
}

/// One pattern segment against one path segment, where `*` matches any run of
/// characters inside the segment.
fn segment_matches(pattern: &str, text: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == text;
    }
    let first = parts[0];
    let last = parts[parts.len() - 1];
    if !text.starts_with(first) || !text.ends_with(last) {
        return false;
    }
    if text.len() < first.len() + last.len() {
        return false;
    }
    let middle = &text[first.len()..text.len() - last.len()];
    let mut rest = middle;
    for part in &parts[1..parts.len() - 1] {
        let Some(index) = rest.find(part) else {
            return false;
        };
        rest = &rest[index + part.len()..];
    }
    parts[1..parts.len() - 1]
        .last()
        .is_none_or(|part| middle.contains(part))
}
