use std::path::Path;

/// Compare complete values without a Debug bound or value formatting. Failed
/// tests retain the private SQLite evidence rather than exposing its rows.
pub fn assert_private_equal<T: PartialEq>(
    left: &T,
    right: &T,
    evidence: &Path,
    context: &'static str,
) {
    assert!(
        left == right,
        "{context}; inspect retained private evidence at {}",
        evidence.display()
    );
}
