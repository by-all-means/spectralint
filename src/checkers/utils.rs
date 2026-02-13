use std::path::Path;

use globset::GlobSet;

use crate::engine::cross_ref::build_glob_set;
use crate::engine::scanner::matches_glob;

pub struct ScopeFilter(Option<GlobSet>);

impl ScopeFilter {
    pub fn new(scope_patterns: &[String]) -> Self {
        Self(if scope_patterns.is_empty() {
            None
        } else {
            Some(build_glob_set(scope_patterns))
        })
    }

    pub fn includes(&self, path: &Path, root: &Path) -> bool {
        self.0
            .as_ref()
            .is_none_or(|set| matches_glob(path, root, set))
    }
}

/// Normalize an identifier by splitting on `_`, `-`, space, and camelCase boundaries,
/// lowercasing all parts, and joining with `_`.
///
/// Handles all-caps acronyms: `HTTPRequest` -> `http_request`, `APIKey` -> `api_key`.
pub fn normalize(name: &str) -> String {
    let mut parts = Vec::new();
    let mut current = String::new();

    let chars: Vec<char> = name.chars().collect();
    let len = chars.len();

    let mut i = 0;
    while i < len {
        let c = chars[i];

        if c == '_' || c == '-' || c == ' ' {
            if !current.is_empty() {
                parts.push(std::mem::take(&mut current));
            }
            i += 1;
            continue;
        }

        if c.is_uppercase() {
            let mut j = i;
            while j < len && chars[j].is_uppercase() {
                j += 1;
            }
            let upper_len = j - i;

            if upper_len > 1 {
                if !current.is_empty() {
                    parts.push(std::mem::take(&mut current));
                }
                if j < len && chars[j].is_lowercase() {
                    let acronym: String = chars[i..j - 1].iter().collect();
                    parts.push(acronym.to_lowercase());
                    current.push(chars[j - 1].to_ascii_lowercase());
                    i = j;
                } else {
                    let acronym: String = chars[i..j].iter().collect();
                    parts.push(acronym.to_lowercase());
                    i = j;
                }
            } else {
                if !current.is_empty() {
                    parts.push(std::mem::take(&mut current));
                }
                current.push(c.to_ascii_lowercase());
                i += 1;
            }
        } else {
            current.push(c.to_ascii_lowercase());
            i += 1;
        }
    }

    if !current.is_empty() {
        parts.push(current);
    }

    parts.join("_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_snake_case() {
        assert_eq!(normalize("api_key"), "api_key");
    }

    #[test]
    fn test_normalize_camel_case() {
        assert_eq!(normalize("apiKey"), "api_key");
    }

    #[test]
    fn test_normalize_kebab_case() {
        assert_eq!(normalize("api-key"), "api_key");
    }

    #[test]
    fn test_normalize_spaces() {
        assert_eq!(normalize("api key"), "api_key");
    }

    #[test]
    fn test_normalize_acronym() {
        assert_eq!(normalize("HTTPRequest"), "http_request");
    }

    #[test]
    fn test_normalize_trailing_acronym() {
        assert_eq!(normalize("requestAPI"), "request_api");
    }

    #[test]
    fn test_normalize_pascal_case() {
        assert_eq!(normalize("ApiKey"), "api_key");
    }

    #[test]
    fn test_normalize_mixed() {
        assert_eq!(normalize("myAPI_key"), "my_api_key");
    }

    // ── Item 20: Normalize edge cases ────────────────────────────────────

    #[test]
    fn test_normalize_mixed_delimiters() {
        assert_eq!(normalize("api_key-name"), "api_key_name");
    }

    #[test]
    fn test_normalize_leading_trailing_delimiters() {
        assert_eq!(normalize("_api_key_"), "api_key");
    }

    #[test]
    fn test_normalize_all_lowercase() {
        assert_eq!(normalize("simple"), "simple");
    }

    #[test]
    fn test_normalize_numbers_in_identifier() {
        assert_eq!(normalize("apiV2"), "api_v2");
    }

    #[test]
    fn test_normalize_empty_string() {
        assert_eq!(normalize(""), "");
    }

    #[test]
    fn test_normalize_single_char() {
        assert_eq!(normalize("A"), "a");
        assert_eq!(normalize("a"), "a");
    }

    #[test]
    fn test_normalize_all_caps() {
        assert_eq!(normalize("API"), "api");
    }

    #[test]
    fn test_normalize_number_after_lowercase() {
        assert_eq!(normalize("api2"), "api2");
    }

    #[test]
    fn test_normalize_all_caps_with_number() {
        assert_eq!(normalize("API2"), "api_2");
    }

    #[test]
    fn test_normalize_camel_with_number_prefix() {
        // Number followed by uppercase letter
        assert_eq!(normalize("api2Key"), "api2_key");
    }
}
