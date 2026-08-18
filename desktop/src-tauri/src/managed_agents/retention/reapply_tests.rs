use super::*;

fn test_db() -> Connection {
    open_retention_db(Path::new(":memory:")).expect("open retention test database")
}

fn confirmed_event() -> RetainedEvent {
    RetainedEvent {
        kind: 30175,
        pubkey: "abc123".to_string(),
        d_tag: "test-persona".to_string(),
        content: r#"{"display_name":"Test"}"#.to_string(),
        created_at: 1000,
        raw_event: "confirmed-event".to_string(),
        pending_sync: false,
    }
}

#[test]
fn exact_confirmed_echo_reapplies_projection() {
    let conn = test_db();
    let confirmed = confirmed_event();
    retain_event(&conn, &confirmed).unwrap();

    assert_eq!(
        retain_inbound_event(&conn, &confirmed).unwrap(),
        InboundOutcome::Reapply
    );

    let row = get_retained_event(&conn, 30175, "abc123", "test-persona")
        .unwrap()
        .unwrap();
    assert!(!row.pending_sync);
    assert_eq!(row.raw_event, confirmed.raw_event);
}

#[test]
fn equal_second_different_confirmed_event_skips() {
    let conn = test_db();
    retain_event(&conn, &confirmed_event()).unwrap();

    let inbound = RetainedEvent {
        content: r#"{"display_name":"Other"}"#.to_string(),
        raw_event: "different-event".to_string(),
        ..confirmed_event()
    };
    assert_eq!(
        retain_inbound_event(&conn, &inbound).unwrap(),
        InboundOutcome::Skipped
    );
}
