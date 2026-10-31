//! Pure std sentinel using the SAME assertion helper as the migration target.
//! No Core, database, Provider, credentials, Cargo or product success evidence.
mod private_assert;

#[test]
fn unequal_raw_secret_marker_is_absent_from_the_real_panic_message() {
    use std::{
        panic,
        path::Path,
        sync::{Arc, Mutex},
    };
    const SECRET: &str = "PRIVATE_RAW_CAPABILITY_SENTINEL_72bdc6";
    let captured = Arc::new(Mutex::new(String::new()));
    let saved = captured.clone();
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let text = info
            .payload()
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| info.payload().downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap();
        *saved.lock().unwrap() = text;
    }));
    let left = vec![vec![SECRET.to_string()]];
    let right = vec![vec!["different original bytes".to_string()]];
    let result = panic::catch_unwind(|| {
        private_assert::assert_private_equal(
            &left,
            &right,
            Path::new("/private/generated-evidence/history.sqlite3"),
            "Original raw records changed",
        )
    });
    panic::set_hook(previous);
    assert!(result.is_err());
    let message = captured.lock().unwrap();
    assert!(!message.contains(SECRET));
    assert!(!message.contains("different original bytes"));
    assert!(message.contains("Original raw records changed"));
    assert!(message.contains("/private/generated-evidence/history.sqlite3"));
}
