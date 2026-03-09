#[doc(hidden)]
#[macro_export]
macro_rules! emit {
    // ── Full span with fix ──────────────────────────────────────────────
    ($result:expr, $file:expr, $line:expr, $severity:expr, $category:expr,
     col: $col:expr, end: ($end_line:expr, $end_col:expr),
     fix: $fix:expr, suggest: $suggestion:expr, $($msg:tt)+) => {
        $result.diagnostics.push($crate::types::Diagnostic {
            file: $file.clone(),
            line: $line,
            column: Some($col),
            end_line: Some($end_line),
            end_column: Some($end_col),
            severity: $severity,
            category: $category,
            message: format!($($msg)+),
            suggestion: Some($suggestion.to_string()),
            fix: Some(Box::new($fix)),
        });
    };
    // ── Full span with suggest ──────────────────────────────────────────
    ($result:expr, $file:expr, $line:expr, $severity:expr, $category:expr,
     col: $col:expr, end: ($end_line:expr, $end_col:expr),
     suggest: $suggestion:expr, $($msg:tt)+) => {
        $result.diagnostics.push($crate::types::Diagnostic {
            file: $file.clone(),
            line: $line,
            column: Some($col),
            end_line: Some($end_line),
            end_column: Some($end_col),
            severity: $severity,
            category: $category,
            message: format!($($msg)+),
            suggestion: Some($suggestion.to_string()),
            fix: None,
        });
    };
    // ── Full span (no suggest/fix) ──────────────────────────────────────
    ($result:expr, $file:expr, $line:expr, $severity:expr, $category:expr,
     col: $col:expr, end: ($end_line:expr, $end_col:expr), $($msg:tt)+) => {
        $result.diagnostics.push($crate::types::Diagnostic {
            file: $file.clone(),
            line: $line,
            column: Some($col),
            end_line: Some($end_line),
            end_column: Some($end_col),
            severity: $severity,
            category: $category,
            message: format!($($msg)+),
            suggestion: None,
            fix: None,
        });
    };
    // ── Column only with suggest ────────────────────────────────────────
    ($result:expr, $file:expr, $line:expr, $severity:expr, $category:expr,
     col: $col:expr, suggest: $suggestion:expr, $($msg:tt)+) => {
        $result.diagnostics.push($crate::types::Diagnostic {
            file: $file.clone(),
            line: $line,
            column: Some($col),
            end_line: None,
            end_column: None,
            severity: $severity,
            category: $category,
            message: format!($($msg)+),
            suggestion: Some($suggestion.to_string()),
            fix: None,
        });
    };
    // ── Column only (no suggest/fix) ────────────────────────────────────
    ($result:expr, $file:expr, $line:expr, $severity:expr, $category:expr,
     col: $col:expr, $($msg:tt)+) => {
        $result.diagnostics.push($crate::types::Diagnostic {
            file: $file.clone(),
            line: $line,
            column: Some($col),
            end_line: None,
            end_column: None,
            severity: $severity,
            category: $category,
            message: format!($($msg)+),
            suggestion: None,
            fix: None,
        });
    };
    // ── Original: fix + suggest ─────────────────────────────────────────
    ($result:expr, $file:expr, $line:expr, $severity:expr, $category:expr, fix: $fix:expr, suggest: $suggestion:expr, $($msg:tt)+) => {
        $result.diagnostics.push($crate::types::Diagnostic {
            file: $file.clone(),
            line: $line,
            column: None,
            end_line: None,
            end_column: None,
            severity: $severity,
            category: $category,
            message: format!($($msg)+),
            suggestion: Some($suggestion.to_string()),
            fix: Some(Box::new($fix)),
        });
    };
    // ── Original: suggest only ──────────────────────────────────────────
    ($result:expr, $file:expr, $line:expr, $severity:expr, $category:expr, suggest: $suggestion:expr, $($msg:tt)+) => {
        $result.diagnostics.push($crate::types::Diagnostic {
            file: $file.clone(),
            line: $line,
            column: None,
            end_line: None,
            end_column: None,
            severity: $severity,
            category: $category,
            message: format!($($msg)+),
            suggestion: Some($suggestion.to_string()),
            fix: None,
        });
    };
    // ── Original: bare (no suggest, no fix) ─────────────────────────────
    ($result:expr, $file:expr, $line:expr, $severity:expr, $category:expr, $($msg:tt)+) => {
        $result.diagnostics.push($crate::types::Diagnostic {
            file: $file.clone(),
            line: $line,
            column: None,
            end_line: None,
            end_column: None,
            severity: $severity,
            category: $category,
            message: format!($($msg)+),
            suggestion: None,
            fix: None,
        });
    };
}
