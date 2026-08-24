use super::*;

#[test]
fn pending_sync_orders_definitions_and_teams_before_identities() {
    let conn = open_retention_db(std::path::Path::new(":memory:")).unwrap();
    for (kind, d_tag, created_at) in [
        (30177, "agent", 1),
        (30176, "team", 2),
        (30175, "definition", 3),
    ] {
        retain_event(
            &conn,
            &RetainedEvent {
                kind,
                pubkey: "abc123".to_string(),
                d_tag: d_tag.to_string(),
                content: "{}".to_string(),
                created_at,
                raw_event: format!(r#"{{"id":"{d_tag}"}}"#),
                pending_sync: true,
            },
        )
        .unwrap();
    }

    let kinds: Vec<_> = get_pending_sync(&conn)
        .unwrap()
        .into_iter()
        .map(|row| row.kind)
        .collect();
    assert_eq!(kinds, vec![30175, 30176, 30177]);
}
