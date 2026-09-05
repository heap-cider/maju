import 'package:maju/shared/push/push_snapshot.dart';
import 'package:maju/shared/push/push_subscription.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('push community snapshot carries flattened resolution policies', () {
    final subscription = buildDesiredMajuPushSubscriptions(
      myPubkey: 'a' * 64,
    ).single;
    final snapshot = MajuPushCommunitySnapshot(
      id: 'community',
      name: 'Team',
      relayUrl: 'https://relay.example.com',
      pubkey: 'a' * 64,
      subscriptions: [subscription],
    );

    final decoded = MajuPushCommunitySnapshot.fromJson(snapshot.toJson());

    expect(decoded.toJson(), snapshot.toJson());
    expect(decoded.subscriptions, hasLength(1));
  });
}
