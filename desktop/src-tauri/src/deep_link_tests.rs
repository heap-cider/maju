use url::Url;

use super::{
    parse_add_community_deep_link, parse_channel_deep_link, parse_entity_deep_link,
    parse_join_deep_link, parse_message_deep_link, parse_nostr_bind_deep_link,
    PendingCommunityDeepLink, PendingCommunityDeepLinks, PendingEntityDeepLinks,
    PendingNavigationDeepLink, PendingNavigationDeepLinks, ENTITY_LINK_TABS,
};

fn entity_link_golden() -> serde_json::Value {
    serde_json::from_str(include_str!("../../../test-fixtures/entity-links.json"))
        .expect("valid entity-links golden fixture")
}

#[test]
fn parse_entity_deep_link_accepts_every_share_link_shape() {
    let golden = entity_link_golden();
    let owner = golden["owner"].as_str().unwrap();
    let dtag = golden["dtag"].as_str().unwrap();
    for raw in golden["links"]
        .as_object()
        .unwrap()
        .values()
        .map(|value| value.as_str().unwrap().to_owned())
        .chain(golden["tabs"].as_array().unwrap().iter().map(|tab| {
            format!(
                "maju://repo?owner={owner}&d={dtag}&tab={}",
                tab.as_str().unwrap()
            )
        }))
    {
        assert!(
            parse_entity_deep_link(&Url::parse(&raw).unwrap()).is_some(),
            "{raw}"
        );
    }
    let expected_tabs = golden["tabs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tab| tab.as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(ENTITY_LINK_TABS.as_slice(), expected_tabs);
}

#[test]
fn parse_entity_deep_link_rejects_malformed_and_non_canonical_links() {
    let golden = entity_link_golden();
    let owner = golden["owner"].as_str().unwrap();
    let event_id = golden["eventId"].as_str().unwrap();
    for raw in [
        // Missing or malformed identifiers.
        format!("maju://repo?owner={owner}"),
        "maju://repo?owner=nope&d=maju-world".to_owned(),
        format!("maju://repo?owner={owner}&d=.hidden"),
        format!("maju://repo?owner={owner}&d=has%20space"),
        format!("maju://pr?owner={owner}&d=maju-world"),
        format!("maju://pr?id=short&owner={owner}&d=maju-world"),
        // Coordinate links take no event id.
        format!("maju://repo?id={event_id}&owner={owner}&d=maju-world"),
        // Non-canonical: unknown param, duplicate param, path, fragment.
        format!("maju://repo?owner={owner}&d=maju-world&relay=wss%3A%2F%2Fx.example"),
        format!("maju://repo?owner={owner}&owner={owner}&d=maju-world"),
        // Unknown tab value, duplicate tab, and tab on an event link.
        format!("maju://repo?owner={owner}&d=maju-world&tab=overview"),
        format!("maju://repo?owner={owner}&d=maju-world&tab=prs&tab=prs"),
        format!("maju://pr?id={event_id}&owner={owner}&d=maju-world&tab=prs"),
        format!("maju://repo/extra?owner={owner}&d=maju-world"),
        format!("maju://repo?owner={owner}&d=maju-world#top"),
        // Not an entity host.
        format!("maju://message?owner={owner}&d=maju-world"),
    ] {
        assert!(
            parse_entity_deep_link(&Url::parse(&raw).unwrap()).is_none(),
            "{raw}"
        );
    }
}

fn pending(id: &str, relay_url: &str, code: Option<&str>) -> PendingCommunityDeepLink {
    PendingCommunityDeepLink {
        id: id.to_owned(),
        kind: if code.is_some() { "join" } else { "connect" }.to_owned(),
        relay_url: relay_url.to_owned(),
        code: code.map(str::to_owned),
        policy_receipt: None,
        name: None,
    }
}

fn pending_navigation(
    id: &str,
    kind: &str,
    channel_id: &str,
    message_id: Option<&str>,
    thread_root_id: Option<&str>,
) -> PendingNavigationDeepLink {
    PendingNavigationDeepLink {
        id: id.to_owned(),
        kind: kind.to_owned(),
        channel_id: channel_id.to_owned(),
        message_id: message_id.map(str::to_owned),
        thread_root_id: thread_root_id.map(str::to_owned),
    }
}

#[test]
fn pending_navigation_links_are_fifo_acknowledged_and_deduplicated() {
    let queue = PendingNavigationDeepLinks::default();
    queue.enqueue(pending_navigation(
        "first",
        "channel",
        "channel-1",
        None,
        None,
    ));
    queue.enqueue(pending_navigation(
        "duplicate",
        "channel",
        "channel-1",
        None,
        None,
    ));
    queue.enqueue(pending_navigation(
        "second",
        "message",
        "channel-1",
        Some("message-1"),
        Some("root-1"),
    ));

    assert_eq!(queue.first().unwrap().id, "first");
    assert!(!queue.acknowledge("second"));
    assert!(queue.acknowledge("first"));
    assert_eq!(queue.first().unwrap().id, "second");
    assert!(queue.acknowledge("second"));
    assert!(queue.first().is_none());
}

#[test]
fn pending_navigation_links_can_be_cleared() {
    let queue = PendingNavigationDeepLinks::default();
    queue.enqueue(pending_navigation(
        "first",
        "channel",
        "channel-1",
        None,
        None,
    ));
    queue.enqueue(pending_navigation(
        "second",
        "message",
        "channel-1",
        Some("message-1"),
        None,
    ));

    queue.clear();
    assert!(queue.first().is_none());
}

#[test]
fn pending_navigation_queue_recovers_after_mutex_poisoning() {
    let queue = std::sync::Arc::new(PendingNavigationDeepLinks::default());
    let poisoner = std::sync::Arc::clone(&queue);
    assert!(std::thread::spawn(move || {
        let _guard = poisoner.0.lock().unwrap();
        panic!("poison queue for recovery regression");
    })
    .join()
    .is_err());

    queue.enqueue(pending_navigation(
        "after-poison",
        "channel",
        "channel-1",
        None,
        None,
    ));
    assert_eq!(queue.first().unwrap().id, "after-poison");
    assert!(queue.acknowledge("after-poison"));
    assert!(queue.first().is_none());
}

#[test]
fn pending_join_serializes_policy_receipt_for_cold_launch_recovery() {
    let mut link = pending("join", "wss://relay.example", Some("invite"));
    link.policy_receipt = Some("relay-signed-receipt".to_owned());

    let payload = serde_json::to_value(link).unwrap();
    assert_eq!(payload["policyReceipt"], "relay-signed-receipt");
}

#[test]
fn pending_community_links_are_fifo_and_acknowledged_in_order() {
    let queue = PendingCommunityDeepLinks::default();
    queue.enqueue(pending("first", "wss://one.example", Some("one")));
    queue.enqueue(pending("second", "wss://two.example", Some("two")));
    assert_eq!(queue.first().unwrap().id, "first");
    assert!(!queue.acknowledge("second"));
    assert!(queue.acknowledge("first"));
    assert_eq!(queue.first().unwrap().id, "second");
}

#[test]
fn pending_community_links_dedupe_exact_intents() {
    let queue = PendingCommunityDeepLinks::default();
    queue.enqueue(pending("first", "wss://one.example", Some("one")));
    queue.enqueue(pending("duplicate", "wss://one.example", Some("one")));
    assert!(queue.acknowledge("first"));
    assert!(queue.first().is_none());
}

#[test]
fn pending_entity_links_survive_until_acknowledged_in_order() {
    let queue = PendingEntityDeepLinks::default();
    let first = queue.enqueue("maju://project?owner=aa&d=first".to_owned());
    let second = queue.enqueue("maju://project?owner=aa&d=second".to_owned());

    assert_eq!(queue.first(), Some(first.clone()));
    assert!(!queue.acknowledge(&second.id));
    assert!(queue.acknowledge(&first.id));
    assert_eq!(queue.first(), Some(second));
}

#[test]
fn pending_entity_links_dedupe_launch_and_open_callbacks() {
    let queue = PendingEntityDeepLinks::default();
    let href = "maju://project?owner=aa&d=maju".to_owned();
    let first = queue.enqueue(href.clone());
    let duplicate = queue.enqueue(href);

    assert_eq!(duplicate.id, first.id);
    assert!(queue.acknowledge(&first.id));
    assert!(queue.first().is_none());
}

fn valid_nostr_bind_url() -> Url {
    Url::parse(
        "maju://nostr-bind?challenge_id=550e8400-e29b-41d4-a716-446655440000&nonce=ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghi01234567&verification_code=123456&audience=maju%3Anostr-identity&action=bind_nostr_identity&protocol=maju-nostr-identity&version=1&origin=https%3A%2F%2Fexample.com&expires_at=2999-01-01T00%3A00%3A00Z&return=clipboard",
    )
    .unwrap()
}

#[test]
fn parse_add_community_deep_link_extracts_relay_and_name() {
    let url = Url::parse(
        "maju://add-community?relay=wss%3A%2F%2Facme.communities.maju.xyz&name=Acme%20Team&ignored=value",
    )
    .unwrap();
    let payload = parse_add_community_deep_link(&url).unwrap();
    assert_eq!(payload.relay_url, "wss://acme.communities.maju.xyz");
    assert_eq!(payload.name.as_deref(), Some("Acme Team"));
}

#[test]
fn parse_add_community_deep_link_accepts_an_omitted_or_empty_name() {
    for raw in [
        "maju://add-community?relay=wss%3A%2F%2Facme.example",
        "maju://add-community?relay=wss%3A%2F%2Facme.example&name=",
    ] {
        assert!(parse_add_community_deep_link(&Url::parse(raw).unwrap())
            .unwrap()
            .name
            .is_none());
    }
}

#[test]
fn parse_add_community_deep_link_rejects_invalid_relays() {
    for raw in [
        "maju://add-community",
        "maju://add-community?relay=",
        "maju://add-community?relay=not-a-url",
        "maju://add-community?relay=https%3A%2F%2Facme.example",
        "maju://add-community?relay=wss%3A%2F%2F",
    ] {
        assert!(parse_add_community_deep_link(&Url::parse(raw).unwrap()).is_none());
    }
}

#[test]
fn parse_channel_deep_link_accepts_one_path_segment() {
    let url = Url::parse("maju://channel/580ca78b-9dae-46f3-8854-bd671853ba32").unwrap();
    let payload = parse_channel_deep_link(&url).unwrap();
    assert_eq!(payload["channelId"], "580ca78b-9dae-46f3-8854-bd671853ba32");
}

#[test]
fn parse_channel_deep_link_accepts_message_path() {
    let message_id = "8455293f0123456789abcdef0123456789abcdef0123456789abcdef01234567";
    let url = Url::parse(&format!(
        "maju://channel/a372f080-5961-4535-b1a3-edffface377d/{message_id}"
    ))
    .unwrap();
    let payload = parse_channel_deep_link(&url).unwrap();
    assert_eq!(payload["channelId"], "a372f080-5961-4535-b1a3-edffface377d");
    assert_eq!(payload["messageId"], message_id);
}

#[test]
fn parse_channel_deep_link_accepts_v7_and_normalizes_uppercase() {
    for (raw, expected) in [
        (
            "maju://channel/018fdb5d-3a64-7c35-b5f9-4a23e1f9d2d9",
            "018fdb5d-3a64-7c35-b5f9-4a23e1f9d2d9",
        ),
        (
            "maju://channel/580CA78B-9DAE-46F3-8854-BD671853BA32",
            "580ca78b-9dae-46f3-8854-bd671853ba32",
        ),
    ] {
        let payload = parse_channel_deep_link(&Url::parse(raw).unwrap()).unwrap();
        assert_eq!(payload["channelId"], expected);
    }
}

#[test]
fn parse_channel_deep_link_rejects_malformed_forms() {
    for raw in [
        "maju://channel",
        "maju://channel/",
        "maju://channel/one/two",
        "maju://channel/580ca78b-9dae-46f3-8854-bd671853ba32/not-hex",
        "maju://channel/580ca78b-9dae-46f3-8854-bd671853ba32/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "maju://channel/580ca78b-9dae-46f3-8854-bd671853ba32/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/extra",
        "maju://channel/580ca78b-9dae-46f3-8854-bd671853ba32/",
        "maju://channel/one?extra=true",
        "maju://channel/one#fragment",
        "maju://:pass@channel/580ca78b-9dae-46f3-8854-bd671853ba32/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "maju://channel/not-a-uuid",
        "maju://channel/%2F",
        "maju://channel/%00",
    ] {
        assert!(parse_channel_deep_link(&Url::parse(raw).unwrap()).is_none());
    }
}

#[test]
fn parse_message_deep_link_extracts_required_params() {
    let url = Url::parse("maju://message?channel=abc&id=xyz").unwrap();
    let payload = parse_message_deep_link(&url).expect("required params present");
    assert_eq!(payload["channelId"], "abc");
    assert_eq!(payload["messageId"], "xyz");
    assert!(payload["threadRootId"].is_null());
}

#[test]
fn parse_message_deep_link_accepts_maju_scheme() {
    let url = Url::parse("maju://message?channel=abc&id=xyz").unwrap();
    let payload = parse_message_deep_link(&url).expect("required params present");
    assert_eq!(payload["channelId"], "abc");
    assert_eq!(payload["messageId"], "xyz");
}

#[test]
fn parse_message_deep_link_includes_thread_root() {
    let url = Url::parse("maju://message?channel=abc&id=xyz&thread=root1").unwrap();
    let payload = parse_message_deep_link(&url).expect("required params present");
    assert_eq!(payload["threadRootId"], "root1");
}

#[test]
fn parse_message_deep_link_rejects_missing_id() {
    let url = Url::parse("maju://message?channel=abc").unwrap();
    assert!(parse_message_deep_link(&url).is_none());
}

#[test]
fn parse_message_deep_link_rejects_empty_channel() {
    // Regression: `channel=&id=foo` previously produced channelId: "".
    let url = Url::parse("maju://message?channel=&id=foo").unwrap();
    assert!(parse_message_deep_link(&url).is_none());
}

#[test]
fn parse_message_deep_link_rejects_empty_id() {
    let url = Url::parse("maju://message?channel=abc&id=").unwrap();
    assert!(parse_message_deep_link(&url).is_none());
}

#[test]
fn parse_message_deep_link_treats_empty_thread_as_absent() {
    let url = Url::parse("maju://message?channel=abc&id=xyz&thread=").unwrap();
    let payload = parse_message_deep_link(&url).expect("required params present");
    assert!(payload["threadRootId"].is_null());
}

#[test]
fn parse_join_deep_link_extracts_relay_and_code() {
    let url = Url::parse("maju://join?relay=wss%3A%2F%2Frelay.example&code=abc.def").unwrap();
    let payload = parse_join_deep_link(&url).expect("required params present");
    assert_eq!(payload["relayUrl"], "wss://relay.example");
    assert_eq!(payload["code"], "abc.def");
    assert!(payload["policyReceipt"].is_null());
}

#[test]
fn parse_join_deep_link_extracts_policy_receipt() {
    let url = Url::parse(
        "maju://join?relay=wss%3A%2F%2Frelay.example&code=abc.def&policy_receipt=receipt.value",
    )
    .unwrap();
    let payload = parse_join_deep_link(&url).expect("required params present");
    assert_eq!(payload["policyReceipt"], "receipt.value");
}

#[test]
fn parse_join_deep_link_rejects_missing_code() {
    let url = Url::parse("maju://join?relay=wss%3A%2F%2Frelay.example").unwrap();
    assert!(parse_join_deep_link(&url).is_none());
}

#[test]
fn parse_join_deep_link_rejects_empty_code() {
    let url = Url::parse("maju://join?relay=wss%3A%2F%2Frelay.example&code=").unwrap();
    assert!(parse_join_deep_link(&url).is_none());
}

#[test]
fn parse_join_deep_link_rejects_missing_relay() {
    let url = Url::parse("maju://join?code=abc.def").unwrap();
    assert!(parse_join_deep_link(&url).is_none());
}

#[test]
fn parse_join_deep_link_rejects_non_websocket_relay() {
    let url = Url::parse("maju://join?relay=https%3A%2F%2Frelay.example&code=abc.def").unwrap();
    assert!(parse_join_deep_link(&url).is_none());
}

#[test]
fn parse_nostr_bind_deep_link_accepts_valid_url() {
    let payload = parse_nostr_bind_deep_link(&valid_nostr_bind_url()).unwrap();
    assert_eq!(payload.challenge_id, "550e8400-e29b-41d4-a716-446655440000");
    assert_eq!(payload.nonce, "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghi01234567");
    assert_eq!(payload.verification_code, "123456");
    assert_eq!(payload.audience, "maju:nostr-identity");
    assert_eq!(payload.action, "bind_nostr_identity");
    assert_eq!(payload.protocol, "maju-nostr-identity");
    assert_eq!(payload.version, "1");
    assert_eq!(payload.origin, "https://example.com");
    assert_eq!(payload.expires_at, "2999-01-01T00:00:00Z");
    assert_eq!(payload.return_mode, "clipboard");
    assert_eq!(payload.callback_url, None);
}

#[test]
fn parse_nostr_bind_deep_link_accepts_same_origin_callback_url() {
    let url = Url::parse("maju://nostr-bind?challenge_id=550e8400-e29b-41d4-a716-446655440000&nonce=ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghi01234567&verification_code=123456&audience=maju%3Anostr-identity&action=bind_nostr_identity&protocol=maju-nostr-identity&version=1&origin=https%3A%2F%2Fexample.com&expires_at=2999-01-01T00%3A00%3A00Z&return=clipboard&callback_url=https%3A%2F%2Fexample.com%2Fmaju%3FmockSession%3D1").unwrap();
    let payload = parse_nostr_bind_deep_link(&url).unwrap();
    assert_eq!(
        payload.callback_url.as_deref(),
        Some("https://example.com/maju?mockSession=1")
    );
}

#[test]
fn parse_nostr_bind_deep_link_accepts_browser_fragment_return() {
    let url = Url::parse("maju://nostr-bind?challenge_id=550e8400-e29b-41d4-a716-446655440000&nonce=ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghi01234567&verification_code=123456&audience=maju%3Anostr-identity&action=bind_nostr_identity&protocol=maju-nostr-identity&version=1&origin=https%3A%2F%2Fexample.com&expires_at=2999-01-01T00%3A00%3A00Z&return=browser_fragment_v1&callback_url=https%3A%2F%2Fexample.com%2Fmaju").unwrap();
    let payload = parse_nostr_bind_deep_link(&url).unwrap();

    assert_eq!(payload.return_mode, "browser_fragment_v1");
    assert_eq!(
        payload.callback_url.as_deref(),
        Some("https://example.com/maju")
    );
}

#[test]
fn parse_nostr_bind_deep_link_requires_callback_for_browser_fragment_return() {
    let url = Url::parse("maju://nostr-bind?challenge_id=550e8400-e29b-41d4-a716-446655440000&nonce=ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghi01234567&verification_code=123456&audience=maju%3Anostr-identity&action=bind_nostr_identity&protocol=maju-nostr-identity&version=1&origin=https%3A%2F%2Fexample.com&expires_at=2999-01-01T00%3A00%3A00Z&return=browser_fragment_v1").unwrap();

    assert_eq!(
        parse_nostr_bind_deep_link(&url).unwrap_err(),
        "browser_fragment_v1 requires callback_url"
    );
}

#[test]
fn parse_nostr_bind_deep_link_rejects_cross_origin_callback_url() {
    let url = Url::parse("maju://nostr-bind?challenge_id=550e8400-e29b-41d4-a716-446655440000&nonce=ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghi01234567&verification_code=123456&audience=maju%3Anostr-identity&action=bind_nostr_identity&protocol=maju-nostr-identity&version=1&origin=https%3A%2F%2Fexample.com&expires_at=2999-01-01T00%3A00%3A00Z&return=clipboard&callback_url=https%3A%2F%2Fevil.example%2Fmaju").unwrap();
    assert!(parse_nostr_bind_deep_link(&url).is_err());
}

#[test]
fn parse_nostr_bind_deep_link_rejects_http_callback_url() {
    let url = Url::parse("maju://nostr-bind?challenge_id=550e8400-e29b-41d4-a716-446655440000&nonce=ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghi01234567&verification_code=123456&audience=maju%3Anostr-identity&action=bind_nostr_identity&protocol=maju-nostr-identity&version=1&origin=https%3A%2F%2Fexample.com&expires_at=2999-01-01T00%3A00%3A00Z&return=clipboard&callback_url=http%3A%2F%2Fexample.com%2Fmaju").unwrap();
    assert!(parse_nostr_bind_deep_link(&url).is_err());
}

#[test]
fn parse_nostr_bind_deep_link_rejects_missing_challenge_id() {
    let url = Url::parse("maju://nostr-bind?nonce=ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghi01234567&verification_code=123456&audience=maju%3Anostr-identity&action=bind_nostr_identity&protocol=maju-nostr-identity&version=1&origin=https%3A%2F%2Fexample.com&expires_at=2999-01-01T00%3A00%3A00Z&return=clipboard").unwrap();
    assert!(parse_nostr_bind_deep_link(&url).is_err());
}

#[test]
fn parse_nostr_bind_deep_link_rejects_empty_nonce() {
    let url = Url::parse("maju://nostr-bind?challenge_id=550e8400-e29b-41d4-a716-446655440000&nonce=&verification_code=123456&audience=maju%3Anostr-identity&action=bind_nostr_identity&protocol=maju-nostr-identity&version=1&origin=https%3A%2F%2Fexample.com&expires_at=2999-01-01T00%3A00%3A00Z&return=clipboard").unwrap();
    assert!(parse_nostr_bind_deep_link(&url).is_err());
}

#[test]
fn parse_nostr_bind_deep_link_rejects_missing_verification_code() {
    let url = Url::parse("maju://nostr-bind?challenge_id=550e8400-e29b-41d4-a716-446655440000&nonce=ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghi01234567&audience=maju%3Anostr-identity&action=bind_nostr_identity&protocol=maju-nostr-identity&version=1&origin=https%3A%2F%2Fexample.com&expires_at=2999-01-01T00%3A00%3A00Z&return=clipboard").unwrap();
    assert!(parse_nostr_bind_deep_link(&url).is_err());
}

#[test]
fn parse_nostr_bind_deep_link_rejects_short_verification_code() {
    let url = Url::parse("maju://nostr-bind?challenge_id=550e8400-e29b-41d4-a716-446655440000&nonce=ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghi01234567&verification_code=12345&audience=maju%3Anostr-identity&action=bind_nostr_identity&protocol=maju-nostr-identity&version=1&origin=https%3A%2F%2Fexample.com&expires_at=2999-01-01T00%3A00%3A00Z&return=clipboard").unwrap();
    assert!(parse_nostr_bind_deep_link(&url).is_err());
}

#[test]
fn parse_nostr_bind_deep_link_rejects_long_verification_code() {
    let url = Url::parse("maju://nostr-bind?challenge_id=550e8400-e29b-41d4-a716-446655440000&nonce=ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghi01234567&verification_code=1234567&audience=maju%3Anostr-identity&action=bind_nostr_identity&protocol=maju-nostr-identity&version=1&origin=https%3A%2F%2Fexample.com&expires_at=2999-01-01T00%3A00%3A00Z&return=clipboard").unwrap();
    assert!(parse_nostr_bind_deep_link(&url).is_err());
}

#[test]
fn parse_nostr_bind_deep_link_rejects_non_digit_verification_code() {
    let url = Url::parse("maju://nostr-bind?challenge_id=550e8400-e29b-41d4-a716-446655440000&nonce=ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghi01234567&verification_code=12345a&audience=maju%3Anostr-identity&action=bind_nostr_identity&protocol=maju-nostr-identity&version=1&origin=https%3A%2F%2Fexample.com&expires_at=2999-01-01T00%3A00%3A00Z&return=clipboard").unwrap();
    assert!(parse_nostr_bind_deep_link(&url).is_err());
}

#[test]
fn parse_nostr_bind_deep_link_rejects_wrong_action() {
    let url = Url::parse("maju://nostr-bind?challenge_id=550e8400-e29b-41d4-a716-446655440000&nonce=ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghi01234567&verification_code=123456&audience=maju%3Anostr-identity&action=wrong&protocol=maju-nostr-identity&version=1&origin=https%3A%2F%2Fexample.com&expires_at=2999-01-01T00%3A00%3A00Z&return=clipboard").unwrap();
    assert!(parse_nostr_bind_deep_link(&url).is_err());
}

#[test]
fn parse_nostr_bind_deep_link_rejects_wrong_audience() {
    let url = Url::parse("maju://nostr-bind?challenge_id=550e8400-e29b-41d4-a716-446655440000&nonce=ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghi01234567&verification_code=123456&audience=other&action=bind_nostr_identity&protocol=maju-nostr-identity&version=1&origin=https%3A%2F%2Fexample.com&expires_at=2999-01-01T00%3A00%3A00Z&return=clipboard").unwrap();
    assert!(parse_nostr_bind_deep_link(&url).is_err());
}

#[test]
fn parse_nostr_bind_deep_link_rejects_non_https_origin() {
    let url = Url::parse("maju://nostr-bind?challenge_id=550e8400-e29b-41d4-a716-446655440000&nonce=ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghi01234567&verification_code=123456&audience=maju%3Anostr-identity&action=bind_nostr_identity&protocol=maju-nostr-identity&version=1&origin=http%3A%2F%2Fexample.com&expires_at=2999-01-01T00%3A00%3A00Z&return=clipboard").unwrap();
    assert!(parse_nostr_bind_deep_link(&url).is_err());
}

#[test]
fn parse_nostr_bind_deep_link_rejects_origin_with_path() {
    let url = Url::parse("maju://nostr-bind?challenge_id=550e8400-e29b-41d4-a716-446655440000&nonce=ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghi01234567&verification_code=123456&audience=maju%3Anostr-identity&action=bind_nostr_identity&protocol=maju-nostr-identity&version=1&origin=https%3A%2F%2Fexample.com%2Fbind&expires_at=2999-01-01T00%3A00%3A00Z&return=clipboard").unwrap();
    assert!(parse_nostr_bind_deep_link(&url).is_err());
}

#[test]
fn parse_nostr_bind_deep_link_rejects_origin_with_credentials() {
    let url = Url::parse("maju://nostr-bind?challenge_id=550e8400-e29b-41d4-a716-446655440000&nonce=ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghi01234567&verification_code=123456&audience=maju%3Anostr-identity&action=bind_nostr_identity&protocol=maju-nostr-identity&version=1&origin=https%3A%2F%2Fuser%40example.com&expires_at=2999-01-01T00%3A00%3A00Z&return=clipboard").unwrap();
    assert!(parse_nostr_bind_deep_link(&url).is_err());
}

#[test]
fn parse_nostr_bind_deep_link_rejects_unsupported_return_mode() {
    let url = Url::parse("maju://nostr-bind?challenge_id=550e8400-e29b-41d4-a716-446655440000&nonce=ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghi01234567&verification_code=123456&audience=maju%3Anostr-identity&action=bind_nostr_identity&protocol=maju-nostr-identity&version=1&origin=https%3A%2F%2Fexample.com&expires_at=2999-01-01T00%3A00%3A00Z&return=callback").unwrap();
    assert!(parse_nostr_bind_deep_link(&url).is_err());
}

#[test]
fn parse_nostr_bind_deep_link_accepts_expired_link_for_user_facing_error() {
    let url = Url::parse("maju://nostr-bind?challenge_id=550e8400-e29b-41d4-a716-446655440000&nonce=ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghi01234567&verification_code=123456&audience=maju%3Anostr-identity&action=bind_nostr_identity&protocol=maju-nostr-identity&version=1&origin=https%3A%2F%2Fexample.com&expires_at=2000-01-01T00%3A00%3A00Z&return=clipboard").unwrap();
    let payload = parse_nostr_bind_deep_link(&url).unwrap();
    assert_eq!(payload.expires_at, "2000-01-01T00:00:00Z");
}
