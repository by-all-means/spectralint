use regex::Regex;
use std::sync::LazyLock;

use crate::config::EmojiDensityConfig;
use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::parser::non_code_lines;
use crate::types::{Category, CheckResult, Severity};

use super::Checker;

pub struct EmojiDensityChecker {
    max_emoji: usize,
}

impl EmojiDensityChecker {
    pub fn new(config: &EmojiDensityConfig) -> Self {
        Self {
            max_emoji: config.max_emoji,
        }
    }
}

/// Matches common visual emoji: pictographs, symbols, dingbats, and emoticons.
/// Excludes ASCII digits and # which are technically in the Emoji category.
static EMOJI_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        "[",
        "\u{2600}-\u{27BF}",   // Misc symbols & dingbats
        "\u{1F300}-\u{1F9FF}", // Misc symbols, emoticons, supplemental
        "\u{1FA00}-\u{1FAFF}", // Chess, symbols, extended-A
        "\u{2B50}\u{2B55}",    // Star, circle
        "\u{23E9}-\u{23F3}",   // Media controls
        "\u{231A}\u{231B}",    // Watch, hourglass
        "\u{25AA}\u{25AB}",    // Squares
        "\u{25FB}-\u{25FE}",   // Medium squares
        "\u{2934}\u{2935}",    // Arrows
        "\u{2B05}-\u{2B07}",   // Arrows
        "]",
    ))
    .unwrap()
});

impl Checker for EmojiDensityChecker {
    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            let mut count = 0;

            for (_, line) in non_code_lines(&file.raw_lines) {
                count += EMOJI_PATTERN.find_iter(line).count();
            }

            if count >= self.max_emoji {
                emit!(
                    result,
                    file.path,
                    1,
                    Severity::Info,
                    Category::EmojiDensity,
                    suggest: "Remove decorative emoji — they consume tokens without adding instruction value",
                    "File contains {} emoji (threshold: {}). Emoji add visual noise without \
                     instruction value for agents.",
                    count,
                    self.max_emoji
                );
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkers::utils::test_helpers::single_file_ctx;

    fn run_check_with_threshold(lines: &[&str], max_emoji: usize) -> CheckResult {
        let (_dir, ctx) = single_file_ctx(lines);
        let config = EmojiDensityConfig {
            enabled: true,
            max_emoji,
        };
        EmojiDensityChecker::new(&config).check(&ctx)
    }

    fn run_check(lines: &[&str]) -> CheckResult {
        run_check_with_threshold(lines, 10)
    }

    #[test]
    fn test_high_emoji_count_detected() {
        let result = run_check(&[
            "# 🚀 Project",
            "## 🎯 Goals",
            "- ✅ Done",
            "- ✅ Also done",
            "- ✅ More done",
            "- ❌ Removed",
            "## 📊 Stats",
            "## ⚡ Performance",
            "## 💡 Tips",
            "## 🔧 Config",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Info);
        assert_eq!(result.diagnostics[0].category, Category::EmojiDensity);
    }

    #[test]
    fn test_low_emoji_count_no_diagnostic() {
        let result = run_check(&["# Project", "## Goals", "- ✅ Done", "- ❌ Skipped"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_no_emoji_no_diagnostic() {
        let result = run_check(&["# Project", "## Goals", "All good."]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_emoji_in_code_block_excluded() {
        let result =
            run_check_with_threshold(&["# Project", "```", "🚀🎯✅❌📊⚡💡🔧📋🗂️📝🔍", "```"], 3);
        assert!(
            result.diagnostics.is_empty(),
            "Emoji inside code blocks should not be counted"
        );
    }

    #[test]
    fn test_custom_threshold() {
        let result = run_check_with_threshold(&["✅ Done", "❌ Skip", "🚀 Go"], 3);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_just_below_threshold() {
        let result = run_check_with_threshold(&["✅ Done", "❌ Skip"], 3);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_ascii_not_counted() {
        // Numbers and # are technically emoji but should not be counted
        let result = run_check_with_threshold(&["Use 1234567890 items", "Section #1 #2 #3"], 1);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_message_includes_count() {
        let result = run_check_with_threshold(&["✅❌🚀🎯📊"], 3);
        assert!(result.diagnostics[0].message.contains('5'));
    }
}
