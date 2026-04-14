//! Formatting helpers for human-readable token/cost/session display.
//!
//! Extracted from cli.rs to enable unit testing and reuse.
//! Uses `unicode-width` for correct terminal column alignment with CJK characters.

use unicode_width::UnicodeWidthStr;

/// Format a token count with human-readable suffixes (K/M/B).
pub fn fmt_tokens(n: u64) -> String {
    if n >= 1_000_000_000 {
        format!("{:.2}B", n as f64 / 1_000_000_000.0)
    } else if n >= 1_000_000 {
        format!("{:.2}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        format!("{}", n)
    }
}

/// Format a count with thousands separators.
pub fn fmt_count(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.2}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{},{:03}", n / 1_000, n % 1_000)
    } else {
        format!("{}", n)
    }
}

/// Format a cost as USD string.
pub fn fmt_cost(cost: f64) -> String {
    format!("${:.2}", cost)
}

/// Format a percentage.
pub fn fmt_pct(pct: f64) -> String {
    format!("{:.1}%", pct)
}

/// Truncate a session ID to fit within `max_len` display columns, appending ellipsis if needed.
/// Uses unicode display width so CJK characters (2 columns each) are measured correctly.
pub fn truncate_session_id(id: &str, max_len: usize) -> String {
    if id.width() <= max_len {
        id.to_string()
    } else {
        // Take chars until we'd exceed max_len - 1 columns (leaving room for …)
        let mut width = 0;
        let truncated: String = id
            .chars()
            .take_while(|c| {
                width += unicode_width::UnicodeWidthChar::width(*c).unwrap_or(0);
                width < max_len
            })
            .collect();
        format!("{}…", truncated)
    }
}

/// Extract a human-readable short name from a full project path.
///
/// `/Users/x/.claude/projects/-Users-x-Desktop-QC-QCliHub` → `Desktop-QC-QCliHub`
pub fn short_project_name(full_path: &str) -> String {
    let name = full_path.rsplit('/').next().unwrap_or(full_path);
    // Strip leading `-Users-<username>-` prefix
    if let Some(rest) = name.strip_prefix('-')
        && let Some(after_users) = rest.strip_prefix("Users-")
        && let Some(pos) = after_users.find('-')
    {
        let project_part = &after_users[pos + 1..];
        if !project_part.is_empty() {
            return project_part.to_string();
        }
    }
    name.to_string()
}

/// Pad/truncate a string to fit exactly `width` terminal columns (left-aligned).
/// Uses unicode display width: CJK characters count as 2 columns, ASCII as 1.
pub fn pad(s: &str, width: usize) -> String {
    let display_width = s.width();
    if display_width > width {
        // Truncate by display width, leaving room for ellipsis (1 column)
        let mut w = 0;
        let truncated: String = s
            .chars()
            .take_while(|c| {
                w += unicode_width::UnicodeWidthChar::width(*c).unwrap_or(0);
                w < width
            })
            .collect();
        format!("{}…", truncated)
    } else {
        let padding = width - display_width;
        format!("{}{}", s, " ".repeat(padding))
    }
}

/// Escape a value for CSV output (wrap in quotes if it contains commas, quotes, or newlines).
pub fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- fmt_tokens ---

    #[test]
    fn test_fmt_tokens_billions() {
        assert_eq!(fmt_tokens(2_500_000_000), "2.50B");
    }

    #[test]
    fn test_fmt_tokens_millions() {
        assert_eq!(fmt_tokens(1_234_567), "1.23M");
    }

    #[test]
    fn test_fmt_tokens_thousands() {
        assert_eq!(fmt_tokens(45_678), "45.7K");
    }

    #[test]
    fn test_fmt_tokens_boundary_million() {
        // Exact boundary: 999,999 should be K, 1,000,000 should be M
        assert_eq!(fmt_tokens(999_999), "1000.0K");
        assert_eq!(fmt_tokens(1_000_000), "1.00M");
    }

    // --- fmt_count ---

    #[test]
    fn test_fmt_count_with_thousands() {
        assert_eq!(fmt_count(1_234), "1,234");
        assert_eq!(fmt_count(42_007), "42,007");
    }

    #[test]
    fn test_fmt_count_small() {
        assert_eq!(fmt_count(42), "42");
    }

    // --- fmt_cost, fmt_pct ---

    #[test]
    fn test_fmt_cost() {
        assert_eq!(fmt_cost(12.5), "$12.50");
        assert_eq!(fmt_cost(0.003), "$0.00");
    }

    #[test]
    fn test_fmt_pct() {
        assert_eq!(fmt_pct(99.95), "100.0%");
        assert_eq!(fmt_pct(33.333), "33.3%");
    }

    // --- truncate_session_id ---

    #[test]
    fn test_truncate_session_id_exact_length() {
        let id = "abcdef";
        assert_eq!(truncate_session_id(id, 6), "abcdef");
    }

    #[test]
    fn test_truncate_session_id_over_length() {
        let id = "abcdefghij";
        let result = truncate_session_id(id, 6);
        assert_eq!(result, "abcde…");
    }

    // --- short_project_name ---

    #[test]
    fn test_short_project_name_standard() {
        let path = "/Users/x/.claude/projects/-Users-x-Desktop-QC-QCliHub";
        assert_eq!(short_project_name(path), "Desktop-QC-QCliHub");
    }

    #[test]
    fn test_short_project_name_no_users_prefix() {
        let path = "/some/other/path/my-project";
        assert_eq!(short_project_name(path), "my-project");
    }

    // --- csv_escape ---

    #[test]
    fn test_csv_escape_with_comma() {
        assert_eq!(csv_escape("hello,world"), "\"hello,world\"");
    }

    #[test]
    fn test_csv_escape_with_quote() {
        assert_eq!(csv_escape(r#"say "hi""#), r#""say ""hi""""#);
    }

    #[test]
    fn test_csv_escape_plain() {
        assert_eq!(csv_escape("simple"), "simple");
    }
}
