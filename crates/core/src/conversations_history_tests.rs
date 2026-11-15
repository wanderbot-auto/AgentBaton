use super::*;
use crate::conversation_tests::{fixture, Fixture};
use crate::storage::{encode, now};
use serde_json::json;
fn request(id: &str, limit: u32) -> ConversationHistoryRequest {
    ConversationHistoryRequest {
        conversation_id: id.into(),
        cursor: None,
        limit,
    }
}
fn append(f: &mut Fixture, id: &str, text: &str) -> u64 {
    let mut c = f.store.get_conversation(id).unwrap();
    c.revision += 1;
    let event = Event {
        sequence: 0,
        event_id: Uuid::new_v4().to_string(),
        aggregate_id: id.into(),
        revision: c.revision,
        kind: "conversation.update".into(),
        occurred_at: now().unwrap(),
        payload: json!({"update":{"kind":{"kind":"agent_text","text":text}}}),
        source: "conversation_core".into(),
    };
    let tx = f.store.conn.transaction().unwrap();
    tx.execute(
        "UPDATE conversations SET document=?1 WHERE id=?2",
        params![encode(&c).unwrap(), id],
    )
    .unwrap();
    tx.execute(
        "INSERT INTO events(document) VALUES(?1)",
        [encode(&event).unwrap()],
    )
    .unwrap();
    let sequence = tx.last_insert_rowid() as u64;
    tx.commit().unwrap();
    sequence
}
fn unrelated(f: &mut Fixture, count: usize, text: &str) {
    let tx = f.store.conn.transaction().unwrap();
    let mut statement = tx
        .prepare("INSERT INTO events(document) VALUES(?1)")
        .unwrap();
    for _ in 0..count {
        let event = Event {
            sequence: 0,
            event_id: Uuid::new_v4().to_string(),
            aggregate_id: "foreign/path scoped actor".repeat(12),
            revision: 0,
            kind: "other.domain".into(),
            occurred_at: now().unwrap(),
            payload: json!({"text":text}),
            source: "other".into(),
        };
        statement.execute([encode(&event).unwrap()]).unwrap();
    }
    drop(statement);
    tx.commit().unwrap();
}
#[test]
fn independent_latest_pages_preserve_global_order_and_fixed_snapshot() {
    let mut f = fixture();
    let c = f.create("chat");
    append(&mut f, &c.id, "one");
    unrelated(&mut f, 3, "noise");
    append(&mut f, &c.id, "two");
    append(&mut f, &c.id, "three");
    let mut r = request(&c.id, 2);
    let first = f.store.conversation_history(&r).unwrap();
    if let Ok(path) = std::env::var("AGENTBATON_HISTORY_FIXTURE_OUTPUT") {
        std::fs::write(path, serde_json::to_vec_pretty(&first).unwrap()).unwrap();
    }
    assert_eq!(
        first.events.iter().map(|e| e.revision).collect::<Vec<_>>(),
        vec![3, 4]
    );
    assert!(first.events[0].sequence < first.events[1].sequence);
    assert_eq!(first.snapshot_revision, 4);
    assert!(first.has_more);
    assert_eq!(first.capture, "normalized_partial");
    append(&mut f, &c.id, "after snapshot");
    r.cursor = first.next_cursor.clone();
    let second = f.store.conversation_history(&r).unwrap();
    assert_eq!(second.snapshot_sequence, first.snapshot_sequence);
    assert_eq!(second.snapshot_revision, 4);
    assert_eq!(
        second.events.iter().map(|e| e.revision).collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert!(matches!(second.coverage, HistoryCoverage::Complete));
    assert!(!second.has_more);
    assert!(second.next_cursor.is_none());
    assert_eq!(
        second.returned_bytes,
        serde_json::to_vec(&second.events).unwrap().len()
    );
    let current = f.store.conversation_history(&request(&c.id, 1)).unwrap();
    assert_eq!(current.snapshot_revision, 5);
    assert_eq!(current.events[0].revision, 5);
    assert_eq!(
        f.store
            .conn
            .pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        18
    );
}
#[test]
fn unrelated_flood_returns_explicit_empty_scan_page_then_real_history() {
    let mut f = fixture();
    let c = f.create("chat");
    append(&mut f, &c.id, "never drowned");
    unrelated(&mut f, 2055, "");
    let mut r = request(&c.id, 100);
    let page = f.store.conversation_history(&r).unwrap();
    assert!(page.events.is_empty());
    assert!(page.has_more && page.scan_limited);
    assert_eq!(page.scanned_rows, 2048);
    assert!(!page.byte_limited);
    assert!(page.gap.is_none());
    let cursor = page.next_cursor.unwrap();
    assert_eq!(cursor.expected_revision, 2);
    assert!(cursor.before_sequence < cursor.snapshot_sequence + 1);
    r.cursor = Some(cursor);
    let next = f.store.conversation_history(&r).unwrap();
    assert_eq!(next.events.len(), 2);
    assert!(matches!(next.coverage, HistoryCoverage::Complete));
    assert!(next.events.iter().all(|e| e.aggregate_id == c.id));
}
#[test]
fn actual_document_bytes_and_response_serialization_are_bounded() {
    let mut f = fixture();
    let c = f.create("chat");
    let body = "\\\"\n🚀".repeat(70000);
    for _ in 0..8 {
        append(&mut f, &c.id, &body);
    }
    let mut r = request(&c.id, 100);
    let mut seen = Vec::new();
    let mut byte_pages = 0;
    loop {
        let page = f.store.conversation_history(&r).unwrap();
        assert!(page.scanned_rows <= MAX_ROWS && page.scanned_bytes <= MAX_BYTES);
        assert!(serde_json::to_vec(&page).unwrap().len() <= MAX_BYTES);
        assert_eq!(
            page.returned_bytes,
            serde_json::to_vec(&page.events).unwrap().len()
        );
        if page.byte_limited {
            byte_pages += 1
        }
        seen.extend(page.events.iter().rev().map(|e| e.revision));
        if !page.has_more {
            assert!(matches!(page.coverage, HistoryCoverage::Complete));
            break;
        }
        r.cursor = page.next_cursor;
    }
    assert!(byte_pages > 0);
    assert_eq!(seen, (1..=9).rev().collect::<Vec<_>>());
}
#[test]
fn byte_limited_empty_page_advances_over_unrelated_rows_without_claiming_gap() {
    let mut f = fixture();
    let c = f.create("chat");
    unrelated(&mut f, 7, &"x".repeat(700000));
    let mut r = request(&c.id, 100);
    let first = f.store.conversation_history(&r).unwrap();
    assert!(first.events.is_empty() && first.byte_limited && first.has_more);
    assert!(first.gap.is_none());
    r.cursor = first.next_cursor;
    let next = f.store.conversation_history(&r).unwrap();
    assert!(matches!(next.coverage, HistoryCoverage::Complete));
    assert_eq!(next.events[0].revision, 1);
}
#[test]
fn missing_middle_and_creation_rows_are_explicit_local_gaps() {
    let mut f = fixture();
    let c = f.create("chat");
    let missing = append(&mut f, &c.id, "missing");
    append(&mut f, &c.id, "newer");
    f.store
        .conn
        .execute("DELETE FROM events WHERE sequence=?1", [missing as i64])
        .unwrap();
    let page = f.store.conversation_history(&request(&c.id, 100)).unwrap();
    assert_eq!(page.events.len(), 1);
    let gap = page.gap.unwrap();
    assert_eq!(gap.expected_revision, 2);
    assert_eq!(gap.observed_revision, Some(1));
    assert!(!page.has_more && page.next_cursor.is_none());
    assert!(matches!(page.coverage, HistoryCoverage::Gap));
    f.store.conn.execute("DELETE FROM events WHERE json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.revision')=1",[&c.id]).unwrap();
    let page = f.store.conversation_history(&request(&c.id, 100)).unwrap();
    assert_eq!(page.gap.unwrap().observed_revision, None);
    assert!(!matches!(page.coverage, HistoryCoverage::Complete));
}
#[test]
fn cursor_tampering_cross_owner_and_expired_tokens_are_refused() {
    let mut f = fixture();
    let c = f.create("chat");
    append(&mut f, &c.id, "one");
    append(&mut f, &c.id, "two");
    let original = f
        .store
        .conversation_history(&request(&c.id, 1))
        .unwrap()
        .next_cursor
        .unwrap();
    for mutation in 0..6 {
        let mut cursor = original.clone();
        match mutation {
            0 => cursor.before_sequence -= 1,
            1 => cursor.expected_revision -= 1,
            2 => cursor.snapshot_revision -= 1,
            3 => cursor.attempt_id = "other".into(),
            4 => cursor.anchor.as_mut().unwrap().sha256 = "0".repeat(64),
            _ => cursor.seal = "0".repeat(64),
        }
        let mut r = request(&c.id, 1);
        r.cursor = Some(cursor);
        assert_eq!(
            f.store.conversation_history(&r).unwrap_err().code,
            "STALE_REVISION"
        );
    }
    let mut other = fixture();
    let other_conversation = other.create("other-chat");
    let mut wrong = request(&other_conversation.id, 1);
    wrong.cursor = Some(original.clone());
    assert_eq!(
        other.store.conversation_history(&wrong).unwrap_err().code,
        "STALE_REVISION"
    );
    let mut r = request(&c.id, 1);
    r.cursor = Some(original.clone());
    assert!(f.store.conversation_history(&r).is_ok());
    assert!(f.store.conversation_history(&r).is_ok());
    registrations()
        .lock()
        .unwrap()
        .retain(|entry| entry.token != original.seal);
    assert_eq!(
        f.store.conversation_history(&r).unwrap_err().code,
        "STALE_REVISION"
    );
}
#[test]
fn cursor_anchor_changes_and_journal_rewind_are_not_silent_restarts() {
    let mut f = fixture();
    let c = f.create("chat");
    append(&mut f, &c.id, "one");
    append(&mut f, &c.id, "two");
    let cursor = f
        .store
        .conversation_history(&request(&c.id, 1))
        .unwrap()
        .next_cursor
        .unwrap();
    let anchor = cursor.anchor.as_ref().unwrap().sequence;
    f.store.conn.execute("UPDATE events SET document=json_set(document,'$.payload.changed',true) WHERE sequence=?1",[anchor as i64]).unwrap();
    let mut r = request(&c.id, 1);
    r.cursor = Some(cursor);
    assert_eq!(
        f.store.conversation_history(&r).unwrap_err().code,
        "STALE_REVISION"
    );
}
#[test]
fn malformed_and_oversized_records_are_never_skipped_or_return_nonadvancing_cursor() {
    let mut f = fixture();
    let c = f.create("chat");
    f.store
        .conn
        .execute("INSERT INTO events(document) VALUES('not json')", [])
        .unwrap();
    assert!(f.store.conversation_history(&request(&c.id, 100)).is_err());
    f.store
        .conn
        .execute("DELETE FROM events WHERE document='not json'", [])
        .unwrap();
    unrelated(&mut f, 1, &"x".repeat(MAX_BYTES));
    assert_eq!(
        f.store
            .conversation_history(&request(&c.id, 100))
            .unwrap_err()
            .code,
        "RESOURCE_LIMIT"
    );
}
#[test]
fn complete_local_chain_does_not_claim_complete_provider_capture() {
    let mut f = fixture();
    let c = f.create("chat");
    let seq = append(&mut f, &c.id, "native dropped record");
    f.store.conn.execute("UPDATE events SET document=json_set(document,'$.payload.update.kind',json(?1)) WHERE sequence=?2",params![r#"{"kind":"stream_gap","dropped_records":3}"#,seq as i64]).unwrap();
    let page = f.store.conversation_history(&request(&c.id, 100)).unwrap();
    assert!(matches!(page.coverage, HistoryCoverage::Complete));
    assert_eq!(page.capture, "normalized_partial");
    assert_eq!(
        page.events[1].payload["update"]["kind"]["kind"],
        "stream_gap"
    );
}
#[test]
fn closed_request_and_limit_validation_do_not_mutate_domain_state() {
    let mut f = fixture();
    let c = f.create("chat");
    let before = f.store.get_conversation(&c.id).unwrap().revision;
    for limit in [0, 101, u32::MAX] {
        assert_eq!(
            f.store
                .conversation_history(&request(&c.id, limit))
                .unwrap_err()
                .code,
            "INVALID_REQUEST"
        );
    }
    assert!(serde_json::from_value::<ConversationHistoryRequest>(
        json!({"conversation_id":c.id,"cursor":null,"limit":10,"task_id":"fake"})
    )
    .is_err());
    assert_eq!(f.store.get_conversation(&c.id).unwrap().revision, before);
}

#[test]
fn anchor_budget_cannot_create_an_infinite_nonadvancing_empty_page() {
    let mut f = fixture();
    let c = f.create("chat");
    for _ in 0..2 {
        append(&mut f, &c.id, &"x".repeat(2_300_000));
    }
    let first = f.store.conversation_history(&request(&c.id, 100)).unwrap();
    assert_eq!(first.events.len(), 1);
    assert!(first.has_more && first.byte_limited);
    let mut r = request(&c.id, 100);
    r.cursor = first.next_cursor;
    assert_eq!(
        f.store.conversation_history(&r).unwrap_err().code,
        "RESOURCE_LIMIT"
    );
}
#[test]
fn foreign_source_and_wrong_creation_attempt_cannot_claim_complete_history() {
    let mut f = fixture();
    let c = f.create("chat");
    let seq = append(&mut f, &c.id, "text");
    f.store
        .conn
        .execute(
            "UPDATE events SET document=json_set(document,'$.source','foreign') WHERE sequence=?1",
            [seq as i64],
        )
        .unwrap();
    assert_eq!(
        f.store
            .conversation_history(&request(&c.id, 100))
            .unwrap_err()
            .code,
        "INPUT_CHANGED"
    );
    f.store
        .conn
        .execute("DELETE FROM events WHERE sequence=?1", [seq as i64])
        .unwrap();
    let mut c = f.store.get_conversation(&c.id).unwrap();
    c.revision = 1;
    f.store
        .conn
        .execute(
            "UPDATE conversations SET document=?1 WHERE id=?2",
            params![encode(&c).unwrap(), c.id],
        )
        .unwrap();
    f.store.conn.execute("UPDATE events SET document=json_set(document,'$.payload.attempt_id','different') WHERE json_extract(document,'$.aggregate_id')=?1",[&c.id]).unwrap();
    assert_eq!(
        f.store
            .conversation_history(&request(&c.id, 100))
            .unwrap_err()
            .code,
        "INPUT_CHANGED"
    );
}
#[test]
fn cursor_registry_is_bounded_and_expiration_only_invalidates_read_positions() {
    let mut registry = VecDeque::new();
    for i in 0..1030 {
        register(
            &mut registry,
            CursorRegistration {
                token: i.to_string(),
                document: vec![0; 2048],
                created: Instant::now(),
            },
        );
    }
    assert_eq!(registry.len(), 1024);
    assert_eq!(registry.front().unwrap().token, "6");
    let mut f = fixture();
    let c = f.create("chat");
    append(&mut f, &c.id, "text");
    let cursor = f
        .store
        .conversation_history(&request(&c.id, 1))
        .unwrap()
        .next_cursor
        .unwrap();
    registrations()
        .lock()
        .unwrap()
        .iter_mut()
        .find(|entry| entry.token == cursor.seal)
        .unwrap()
        .created = Instant::now() - Duration::from_secs(1801);
    let mut r = request(&c.id, 1);
    r.cursor = Some(cursor);
    assert_eq!(
        f.store.conversation_history(&r).unwrap_err().code,
        "STALE_REVISION"
    );
    assert_eq!(f.store.get_conversation(&c.id).unwrap().state, "starting");
    assert!(f.store.conversation_history(&request(&c.id, 1)).is_ok());
}
