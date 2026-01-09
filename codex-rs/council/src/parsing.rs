use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Severity {
    P0, // Blocker / Security
    P1, // Logic Error
    P2, // Maintainability
    P3, // Nitpick
    Unknown,
}

impl Severity {
    fn from_str(s: &str) -> Self {
        match s.trim().to_uppercase().as_str() {
            "P0" => Severity::P0,
            "P1" => Severity::P1,
            "P2" => Severity::P2,
            "P3" => Severity::P3,
            _ => Severity::Unknown,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub severity: Severity,
    pub body: String, // Contains the inner text (opaque)
    pub attrs: HashMap<String, String>,
}

/// Extract patch content. Automatically unwraps <![CDATA[ ... ]]> if present.
pub fn extract_patch(text: &str) -> Option<String> {
    let body = extract_first_block(text, "patch")?;
    Some(unwrap_cdata(body))
}

/// Extract plan content.
pub fn extract_plan(text: &str) -> Option<String> {
    extract_first_block(text, "plan").map(|s| s.trim().to_string())
}

/// Extract error content.
pub fn extract_error(text: &str) -> Option<String> {
    extract_first_block(text, "error").map(|s| s.trim().to_string())
}

/// Extract all <finding ...>...</finding> blocks.
pub fn extract_findings(text: &str) -> Vec<Finding> {
    let mut out = Vec::new();
    let mut cursor = 0usize;
    let open_pat = "<finding";
    let close_pat = "</finding>";

    while let Some(open_rel) = text[cursor..].find(open_pat) {
        let open_start = cursor + open_rel;
        let after_open = &text[open_start..];

        // Find end of opening tag '>'
        let gt_rel = match after_open.find('>') {
            Some(v) => v,
            None => break,
        };
        let open_tag_full = &after_open[..=gt_rel];

        let attrs = parse_attrs(open_tag_full);
        // Map string -> Enum immediately
        let sev_str = attrs.get("severity").cloned().unwrap_or_default();
        let severity = Severity::from_str(&sev_str);

        let body_start = open_start + gt_rel + 1;
        let after_body = &text[body_start..];

        let close_rel = match after_body.find(close_pat) {
            Some(v) => v,
            None => break,
        };
        let body_end = body_start + close_rel;

        out.push(Finding {
            severity,
            body: text[body_start..body_end].trim().to_string(),
            attrs,
        });
        cursor = body_end + close_pat.len();
    }
    out
}

/// Validates if the string looks like a valid apply_patch payload.
pub fn looks_like_apply_patch(patch: &str) -> bool {
    let t = patch.trim();
    if !t.contains("*** Begin Patch") || !t.contains("*** End Patch") {
        return false;
    }
    if !(t.contains("*** Add File:")
        || t.contains("*** Update File:")
        || t.contains("*** Delete File:"))
    {
        return false;
    }
    // Basic absolute path guard (Unix + Windows-ish)
    if t.contains("*** Add File: /")
        || t.contains("*** Update File: /")
        || t.contains("*** Delete File: /")
    {
        return false;
    }
    if t.contains("*** Add File: \\")
        || t.contains("*** Update File: \\")
        || t.contains("*** Delete File: \\")
    {
        return false;
    }
    true
}

/// Strictly validates paths in the patch content to prevent path traversal.
/// Returns Ok(()) if safe, Err(String) with the reason if unsafe.
pub fn validate_patch_paths(patch: &str) -> Result<(), String> {
    for line in patch.lines() {
        let trimmed = line.trim();
        let path_str = trimmed
            .strip_prefix("*** Add File: ")
            .or_else(|| trimmed.strip_prefix("*** Update File: "))
            .or_else(|| trimmed.strip_prefix("*** Delete File: "))
            .or_else(|| trimmed.strip_prefix("*** Move to: "));

        let Some(path_str) = path_str else {
            continue;
        };

        validate_patch_path(path_str.trim())?;
    }
    Ok(())
}

fn validate_patch_path(path: &str) -> Result<(), String> {
    if path.is_empty() {
        return Err("Found empty file path in patch".to_string());
    }

    // Reject absolute paths (Unix + Windows-ish) even on non-Windows platforms.
    if path.starts_with('/') || path.starts_with('\\') {
        return Err(format!("Absolute path detected: {path}"));
    }
    let bytes = path.as_bytes();
    if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
        return Err(format!("Absolute path detected (Windows drive): {path}"));
    }

    // Reject path traversal ("..") on both separators.
    for segment in path.split(['/', '\\']) {
        if segment == ".." {
            return Err(format!("Path traversal detected ('..'): {path}"));
        }
    }

    Ok(())
}

// --- Internal Helpers ---

// Helper: Remove <![CDATA[ and ]]> if present
fn unwrap_cdata(s: String) -> String {
    let trimmed = s.trim();
    if let Some(inner) = trimmed.strip_prefix("<![CDATA[")
        && let Some(inner2) = inner.strip_suffix("]]>")
    {
        return inner2.to_string();
    }
    s
}

// Helper: Find first <tag>...</tag> ignoring outer text
fn extract_first_block(text: &str, tag: &str) -> Option<String> {
    let open_pat = format!("<{tag}");
    let close_pat = format!("</{tag}>");

    let open_start = text.find(&open_pat)?;
    let after_open = &text[open_start..];
    let gt_rel = after_open.find('>')?;
    let body_start = open_start + gt_rel + 1;

    let after_body = &text[body_start..];
    let close_rel = after_body.find(&close_pat)?;

    Some(text[body_start..body_start + close_rel].to_string())
}

// Helper: Robust attribute parser (state machine)
fn parse_attrs(open_tag: &str) -> HashMap<String, String> {
    let mut attrs = HashMap::new();
    let mut chars = open_tag.chars().peekable();

    // Skip tag name
    for c in chars.by_ref() {
        if c.is_whitespace() {
            break;
        }
        if c == '>' {
            return attrs;
        }
    }

    loop {
        // Skip whitespace
        while let Some(&c) = chars.peek() {
            if c.is_whitespace() {
                chars.next();
            } else {
                break;
            }
        }

        if let Some(&c) = chars.peek() {
            if c == '>' {
                break;
            }
        } else {
            break;
        }

        // Read key
        let mut key = String::new();
        while let Some(&c) = chars.peek() {
            if c == '=' || c.is_whitespace() || c == '>' {
                break;
            }
            key.push(c);
            chars.next();
        }

        if key.is_empty() {
            break;
        }

        // Skip to equals
        let mut found_eq = false;
        while let Some(&c) = chars.peek() {
            if c == '=' {
                chars.next();
                found_eq = true;
                break;
            }
            if c.is_whitespace() {
                chars.next();
            } else {
                break;
            }
        }

        if !found_eq {
            attrs.insert(key, String::new());
            continue;
        }

        // Read value
        // Skip whitespace after =
        while let Some(&c) = chars.peek() {
            if c.is_whitespace() {
                chars.next();
            } else {
                break;
            }
        }

        let mut val = String::new();
        if let Some(&quote) = chars.peek() {
            if quote == '"' || quote == '\x27' {
                chars.next(); // consume open quote
                for c in chars.by_ref() {
                    if c == quote {
                        break;
                    }
                    val.push(c);
                }
                // Consume closing quote if present
                if let Some(c) = chars.peek()
                    && *c == quote
                {
                    chars.next();
                }
            } else {
                // Unquoted value
                while let Some(&c) = chars.peek() {
                    if c.is_whitespace() || c == '>' {
                        break;
                    }
                    val.push(c);
                    chars.next();
                }
            }
        }
        attrs.insert(key, val);
    }
    attrs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_findings_from_messy_input() {
        let input = r#" 
        Here is my critique:
        <finding severity="P0">
            <issue>SQL Injection</issue>
        </finding>
        Some filler text.
        <finding severity='P2'>
            <issue>Spelling</issue>
        </finding>
        "#;

        let findings = extract_findings(input);
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].severity, Severity::P0);
        assert!(findings[0].body.contains("SQL Injection"));
        assert_eq!(findings[1].severity, Severity::P2);
        assert!(findings[1].body.contains("Spelling"));
    }

    #[test]
    fn test_extract_patch_with_cdata() {
        let input = r#" 
        Sure, here is the patch:
        <patch>
        <![CDATA[
        diff --git a/file.rs b/file.rs
        index 123..456 100644
        --- a/file.rs
        +++ b/file.rs
        @@ -1,1 +1,1 @@
        -old
        +new
        ]]>
        </patch>
        "#;

        let patch = extract_patch(input).expect("Should extract patch");
        assert!(patch.contains("-old"));
        assert!(patch.contains("+new"));
        assert!(!patch.contains("CDATA"));
    }

    #[test]
    fn test_extract_plan() {
        let input = r#" 
        <plan>
        1. Do this.
        2. Do that.
        </plan>
        "#;
        let plan = extract_plan(input).expect("Should extract plan");
        assert_eq!(plan, "1. Do this.\n        2. Do that.");
    }

    #[test]
    fn test_parse_attrs() {
        let tag = r#"<finding severity="P0" type='bug' checked>"#;
        let attrs = parse_attrs(tag);
        assert_eq!(
            attrs.get("severity").map(std::string::String::as_str),
            Some("P0")
        );
        assert_eq!(
            attrs.get("type").map(std::string::String::as_str),
            Some("bug")
        );
        assert_eq!(
            attrs.get("checked").map(std::string::String::as_str),
            Some("")
        );
    }

    #[test]
    fn test_looks_like_apply_patch() {
        let good = "*** Begin Patch\n*** Add File: foo.rs\n*** End Patch";
        assert!(looks_like_apply_patch(good));

        let bad_markers = "Here is the patch:\n*** Add File: foo.rs";
        assert!(!looks_like_apply_patch(bad_markers));

        let bad_abs = "*** Begin Patch\n*** Add File: /etc/passwd\n*** End Patch";
        assert!(!looks_like_apply_patch(bad_abs));
    }

    #[test]
    fn test_validate_patch_paths() {
        let safe = "*** Add File: src/main.rs\n*** Update File: config/settings.toml\n*** Move to: src/new.rs";
        assert!(validate_patch_paths(safe).is_ok());

        let traversal = "*** Add File: ../evil.txt";
        assert!(validate_patch_paths(traversal).is_err());

        let traversal_deep = "*** Update File: src/../../etc/passwd";
        assert!(validate_patch_paths(traversal_deep).is_err());

        let traversal_move = "*** Move to: ../evil.txt";
        assert!(validate_patch_paths(traversal_move).is_err());

        let abs_unix = "*** Delete File: /usr/bin/oops";
        assert!(validate_patch_paths(abs_unix).is_err());

        let abs_win = "*** Add File: C:\\Windows\\System32\\oops.dll";
        assert!(validate_patch_paths(abs_win).is_err());
    }
}
