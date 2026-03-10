#[doc(hidden)]
#[macro_export]
macro_rules! emit {
    // ── fix + suggest ───────────────────────────────────────────────────
    ($result:expr, $file:expr, $line:expr, $severity:expr, $category:expr,
     fix: $fix:expr, suggest: $suggestion:expr, $($msg:tt)+) => {
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
    // ── suggest only ────────────────────────────────────────────────────
    ($result:expr, $file:expr, $line:expr, $severity:expr, $category:expr,
     suggest: $suggestion:expr, $($msg:tt)+) => {
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
    // ── bare (no suggest, no fix) ───────────────────────────────────────
    ($result:expr, $file:expr, $line:expr, $severity:expr, $category:expr,
     $($msg:tt)+) => {
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
