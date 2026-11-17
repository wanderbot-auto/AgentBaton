use super::*;
use std::time::Instant;

#[test]
fn journal_oversized_blob_is_rejected_before_document_type_conversion() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("journal.sqlite3");
    let mut store = Store::open(&path).unwrap();
    store.create("local", "Small event", "key").unwrap();
    let other = Connection::open(&path).unwrap();
    other
        .execute(
            "UPDATE events SET document=zeroblob(?1) WHERE sequence=1",
            [(MAX_EVENT_PAGE_BYTES * 2) as i64],
        )
        .unwrap();
    // SQL returns NULL in its document column; trying String conversion before
    // the byte guard would yield INTERNAL_ERROR rather than RESOURCE_LIMIT.
    let error = store.events(0, 100).unwrap_err();
    assert_eq!(error.code, "RESOURCE_LIMIT");
    assert_eq!(error.details_ref.as_deref(), Some("event-sequence:1"));
}

#[test]
fn journal_batch_keeps_order_empty_tail_and_exact_byte_boundary() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("journal.sqlite3")).unwrap();
    let event = |sequence, payload| Event {
        sequence,
        event_id: format!("e{sequence}"),
        aggregate_id: "task".into(),
        revision: 1,
        kind: "session.output".into(),
        occurred_at: "now".into(),
        payload,
        source: "agent".into(),
    };
    let first = encode(&event(1, json!("雪"))).unwrap();
    let prefix = encode(&event(2, json!(""))).unwrap();
    let second = encode(&event(
        2,
        json!("x".repeat(MAX_EVENT_PAGE_BYTES - first.len() - prefix.len())),
    ))
    .unwrap();
    let third = encode(&event(3, json!("tail"))).unwrap();
    for (sequence, raw) in [(1, &first), (2, &second), (3, &third)] {
        store
            .conn
            .execute(
                "INSERT INTO events(sequence,document) VALUES(?1,?2)",
                params![sequence, raw],
            )
            .unwrap();
    }
    let page = store.events(0, 100).unwrap();
    assert_eq!(page["page_bytes"], MAX_EVENT_PAGE_BYTES);
    assert_eq!(page["byte_limited"], true);
    assert_eq!(page["next_cursor"], 2);
    assert_eq!(page["events"].as_array().unwrap().len(), 2);
    assert_eq!(page["events"][0]["payload"], "雪");
    assert_eq!(page["events"][1]["sequence"], 2);
    let tail = store.events(2, 100).unwrap();
    assert_eq!(tail["next_cursor"], 3);
    assert_eq!(tail["events"][0]["payload"], "tail");
    let empty = store.events(3, 100).unwrap();
    assert_eq!(empty["events"], json!([]));
    assert_eq!(empty["next_cursor"], 3);
    assert_eq!(empty["page_bytes"], 0);
    assert_eq!(empty["byte_limited"], false);
    assert_eq!(store.events(4, 100).unwrap_err().code, "STALE_REVISION");
    // Do not even convert a next-page document when the current page is full.
    // On its own page malformed type must surface as an error, never be skipped.
    store
        .conn
        .execute(
            "UPDATE events SET document=zeroblob(4) WHERE sequence=3",
            [],
        )
        .unwrap();
    let full = store.events(0, 100).unwrap();
    assert_eq!(full["next_cursor"], 2);
    assert_eq!(full["byte_limited"], true);
    assert_eq!(store.events(2, 100).unwrap_err().code, "INTERNAL_ERROR");
}

#[test]
fn journal_page_operation_benchmark() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(&dir.path().join("journal.sqlite3")).unwrap();
    for index in 0..100 {
        store
            .create(
                "local",
                &format!("Task {index}: {}", "x".repeat(2048)),
                &format!("key-{index}"),
            )
            .unwrap();
    }
    let expected = store.events(0, 100).unwrap();
    assert_eq!(expected["events"].as_array().unwrap().len(), 100);
    let mut samples = Vec::new();
    for _ in 0..7 {
        let started = Instant::now();
        for _ in 0..200 {
            let page = store.events(0, 100).unwrap();
            assert_eq!(page["next_cursor"], 100);
            assert_eq!(
                page["events"][99]["payload"],
                expected["events"][99]["payload"]
            );
            std::hint::black_box(page);
        }
        samples.push(started.elapsed().as_secs_f64() * 1000.0 / 200.0);
    }
    samples.sort_by(f64::total_cmp);
    eprintln!(
        "journal_100_events_2KiB_ms_per_page median={:.4} samples={samples:?}",
        samples[3]
    );
}
