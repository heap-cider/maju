use nostr::{EventBuilder, EventId, Kind};
use uuid::Uuid;

use super::tag;

/// Kind 5 — NIP-09 deletion. The `h` tag is non-standard for NIP-09 but is
/// required so channel-scoped subscriptions observe the delete.
pub fn build_delete_compat(
    channel_id: Uuid,
    target_event_id: EventId,
) -> Result<EventBuilder, String> {
    let tags = vec![
        tag(vec!["h", &channel_id.to_string()])?,
        tag(vec!["e", &target_event_id.to_hex()])?,
    ];
    Ok(EventBuilder::new(Kind::Custom(5), "").tags(tags))
}

/// Kind 9005 — Maju-native moderation deletion. Unlike a NIP-09 kind 5
/// deletion, this does not claim that the signer authored the target event;
/// the relay validates the signer's channel/community moderation authority.
pub fn build_moderation_delete(
    channel_id: Uuid,
    target_event_id: EventId,
) -> Result<EventBuilder, String> {
    let tags = vec![
        tag(vec!["h", &channel_id.to_string()])?,
        tag(vec!["e", &target_event_id.to_hex()])?,
    ];
    Ok(EventBuilder::new(Kind::Custom(9005), "").tags(tags))
}

#[cfg(test)]
mod tests {
    use nostr::Keys;

    use super::*;

    #[test]
    fn moderation_delete_uses_native_kind_without_impersonating_author() {
        let channel = Uuid::parse_str("a1b2c3d4-e5f6-7890-abcd-ef1234567890").unwrap();
        let target =
            EventId::from_hex("d24da132115ca0a46233cf4c2ad8338fbf914250cbcaa9181a6dd59533cb5ac1")
                .unwrap();
        let moderator = Keys::new(
            nostr::SecretKey::from_hex(
                "0000000000000000000000000000000000000000000000000000000000000004",
            )
            .unwrap(),
        );
        let event = build_moderation_delete(channel, target)
            .unwrap()
            .sign_with_keys(&moderator)
            .unwrap();
        let tags: Vec<Vec<String>> = event
            .tags
            .iter()
            .map(|tag| tag.as_slice().to_vec())
            .collect();

        assert_eq!(event.kind, Kind::Custom(9005));
        assert_eq!(event.pubkey, moderator.public_key());
        assert_eq!(tags[0], vec!["h", &channel.to_string()]);
        assert_eq!(tags[1], vec!["e", target.to_hex().as_str()]);
    }
}
