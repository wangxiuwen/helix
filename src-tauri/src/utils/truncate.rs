/// Safely truncate a string to at most `max_chars` characters,
/// respecting UTF-8 char boundaries. Returns a `&str` slice.
pub fn safe_truncate(s: &str, max_chars: usize) -> &str {
    if s.len() <= max_chars {
        return s;
    }
    // Find the byte index of the (max_chars)th character
    match s.char_indices().nth(max_chars) {
        Some((idx, _)) => &s[..idx],
        None => s, // fewer chars than max_chars
    }
}
