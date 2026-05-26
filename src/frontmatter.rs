use anyhow::{Context, Result};
use serde_yaml_ng::Mapping;

const FENCE: &str = "---";

/// Split `source` into an optional YAML frontmatter block and the body that
/// follows. A frontmatter block is recognized when `source` begins with a
/// `---` line and contains a later `---` line that closes it. Anything else
/// — body-only files, missing closing fence, empty input — yields
/// `(None, source)`. Malformed frontmatter is treated as no frontmatter,
/// never an error.
pub fn split(source: &str) -> (Option<&str>, &str) {
    let Some(after_open) = strip_fence_line(source) else {
        return (None, source);
    };
    let Some(close_at) = find_close_fence(after_open) else {
        return (None, source);
    };
    let yaml = &after_open[..close_at];
    let after_close = &after_open[close_at + FENCE.len()..];
    let body = after_close.strip_prefix("\r\n").unwrap_or_else(|| {
        after_close.strip_prefix('\n').unwrap_or(after_close)
    });
    (Some(yaml), body)
}

/// Parse a YAML string into a `Mapping`. Empty input parses to an empty map.
pub fn parse_yaml(yaml: &str) -> Result<Mapping> {
    if yaml.trim().is_empty() {
        return Ok(Mapping::new());
    }
    serde_yaml_ng::from_str(yaml).context("could not parse YAML frontmatter")
}

/// Split a source string and parse its frontmatter in one call. Missing or
/// empty frontmatter yields an empty `Mapping`; a present block with
/// malformed YAML yields `Err`.
pub fn parse(source: &str) -> Result<(Mapping, &str)> {
    let (yaml, body) = split(source);
    let data = match yaml {
        Some(y) => parse_yaml(y)?,
        None => Mapping::new(),
    };
    Ok((data, body))
}

fn strip_fence_line(s: &str) -> Option<&str> {
    let rest = s.strip_prefix(FENCE)?;
    if let Some(after) = rest.strip_prefix("\r\n") {
        return Some(after);
    }
    if let Some(after) = rest.strip_prefix('\n') {
        return Some(after);
    }
    if rest.is_empty() {
        return Some(rest);
    }
    None
}

fn find_close_fence(s: &str) -> Option<usize> {
    let mut start = 0;
    while start <= s.len() {
        let line_end = s[start..]
            .find('\n')
            .map(|i| start + i)
            .unwrap_or(s.len());
        let line = &s[start..line_end];
        let trimmed = line.strip_suffix('\r').unwrap_or(line);
        if trimmed == FENCE {
            return Some(start);
        }
        if line_end == s.len() {
            return None;
        }
        start = line_end + 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input() {
        assert_eq!(split(""), (None, ""));
    }

    #[test]
    fn body_only() {
        assert_eq!(split("# Hello\n"), (None, "# Hello\n"));
    }

    #[test]
    fn valid_frontmatter() {
        let source = "---\ntitle: Hi\n---\n# Hello\n";
        let (fm, body) = split(source);
        assert_eq!(fm, Some("title: Hi\n"));
        assert_eq!(body, "# Hello\n");
    }

    #[test]
    fn missing_close_fence_is_graceful() {
        let source = "---\ntitle: Hi\nno closing fence";
        assert_eq!(split(source), (None, source));
    }

    #[test]
    fn crlf_line_endings() {
        let source = "---\r\ntitle: Hi\r\n---\r\n# Hello\r\n";
        let (fm, body) = split(source);
        assert_eq!(fm, Some("title: Hi\r\n"));
        assert_eq!(body, "# Hello\r\n");
    }

    #[test]
    fn empty_frontmatter_block() {
        let source = "---\n---\nbody";
        let (fm, body) = split(source);
        assert_eq!(fm, Some(""));
        assert_eq!(body, "body");
        assert!(parse_yaml(fm.unwrap()).unwrap().is_empty());
    }

    #[test]
    fn parse_yaml_empty_is_empty_map() {
        assert!(parse_yaml("").unwrap().is_empty());
        assert!(parse_yaml("   \n\n").unwrap().is_empty());
    }

    #[test]
    fn parse_yaml_invalid_errors() {
        assert!(parse_yaml("title: [unterminated").is_err());
    }

    #[test]
    fn parse_returns_data_for_present_frontmatter() {
        let (data, body) = parse("---\ntitle: Hi\n---\n# Body\n").unwrap();
        assert_eq!(
            data.get("title").and_then(|v| v.as_str()),
            Some("Hi")
        );
        assert_eq!(body, "# Body\n");
    }

    #[test]
    fn parse_returns_empty_map_for_body_only() {
        let (data, body) = parse("# Just a body\n").unwrap();
        assert!(data.is_empty());
        assert_eq!(body, "# Just a body\n");
    }

    #[test]
    fn parse_errors_on_malformed_yaml() {
        assert!(parse("---\ntitle: [unterminated\n---\nbody").is_err());
    }

    #[test]
    fn fence_with_trailing_text_is_not_a_fence() {
        let source = "---x\ntitle: Hi\n---\nbody";
        assert_eq!(split(source), (None, source));
    }

    #[test]
    fn no_body_after_close_fence() {
        let source = "---\ntitle: Hi\n---";
        let (fm, body) = split(source);
        assert_eq!(fm, Some("title: Hi\n"));
        assert_eq!(body, "");
    }
}
