import 'package:flutter/foundation.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';

import '../community/community_provider.dart';
import '../relay/relay_provider.dart';
import '../relay/relay_session.dart';
import 'dev_push_lease.dart';

typedef MajuPushDescriptorFetcher =
    Future<MajuPushLeaseDescriptor> Function(String relayBaseUrl);

final majuPushDescriptorFetcherProvider = Provider<MajuPushDescriptorFetcher>(
  (ref) => fetchMajuPushLeaseDescriptor,
);

/// The fully validated push capability advertised by the current relay.
///
/// Discovery fails closed. An absent, malformed, or unreachable NIP-11 push
/// descriptor is represented as no capability, so no notification permission,
/// APNs registration, gateway enrollment, or relay lease can begin.
final currentRelayPushDescriptorProvider =
    FutureProvider.autoDispose<MajuPushLeaseDescriptor?>((ref) async {
      final session = ref.watch(relaySessionProvider);
      final config = ref.watch(relayConfigProvider);
      final community = ref.watch(activeCommunityProvider).value;
      final memberPubkey = ref.watch(myPubkeyProvider);
      if (session.status != SessionStatus.connected ||
          community == null ||
          config.nsec == null ||
          config.nsec!.isEmpty ||
          memberPubkey == null ||
          memberPubkey.isEmpty) {
        return null;
      }

      return discoverMajuPushRelayCapability(
        config.baseUrl,
        fetchDescriptor: ref.read(majuPushDescriptorFetcherProvider),
      );
    });

Future<MajuPushLeaseDescriptor?> discoverMajuPushRelayCapability(
  String relayBaseUrl, {
  required MajuPushDescriptorFetcher fetchDescriptor,
}) async {
  try {
    return await fetchDescriptor(relayBaseUrl);
  } catch (error, stackTrace) {
    debugPrint('Current relay does not advertise valid push: $error');
    debugPrintStack(stackTrace: stackTrace);
    return null;
  }
}

Future<void> startMajuPushRegistrationIfCapable(
  MajuPushLeaseDescriptor? descriptor, {
  required Future<void> Function() startRegistration,
}) async {
  if (descriptor == null) return;
  await startRegistration();
}
