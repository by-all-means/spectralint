#[macro_export]
macro_rules! emit {
    ($result:expr, $file:expr, $line:expr, $severity:expr, $category:expr, suggest: $suggestion:expr, $($msg:tt)+) => {
        $result.diagnostics.push($crate::types::Diagnostic {
            file: $file.to_path_buf(),
            line: $line,
            severity: $severity,
            category: $category,
            message: format!($($msg)+),
            suggestion: Some($suggestion.to_string()),
        });
    };
    ($result:expr, $file:expr, $line:expr, $severity:expr, $category:expr, $($msg:tt)+) => {
        $result.diagnostics.push($crate::types::Diagnostic {
            file: $file.to_path_buf(),
            line: $line,
            severity: $severity,
            category: $category,
            message: format!($($msg)+),
            suggestion: None,
        });
    };
}
