part of '../settings_page.dart';

class _NotificationsSection extends ConsumerWidget {
  const _NotificationsSection();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    if (defaultTargetPlatform != TargetPlatform.iOS) {
      return const SizedBox.shrink();
    }
    final community = ref.watch(activeCommunityProvider).value;
    if (community == null) return const SizedBox.shrink();
    final authorization = ref.watch(majuPushAuthorizationStatusProvider);
    final status = authorization.value;
    final permissionUnavailable = authorization.hasError;
    final permissionDenied = status == MajuPushAuthorizationStatus.denied;
    final showSettingsRecovery =
        community.pushNotificationsEnabled &&
        (permissionDenied || permissionUnavailable);
    final subtitle = !community.pushNotificationsEnabled
        ? 'Off for this community'
        : switch (status) {
            MajuPushAuthorizationStatus.notDetermined =>
              'Waiting for iOS notification permission',
            MajuPushAuthorizationStatus.denied =>
              'Enabled in Maju, but disabled in iOS Settings',
            MajuPushAuthorizationStatus.authorized ||
            MajuPushAuthorizationStatus.provisional ||
            MajuPushAuthorizationStatus.ephemeral =>
              'Receive message notifications from this community',
            null when authorization.isLoading =>
              'Checking iOS notification permission',
            null => 'Enabled in Maju; iOS permission status unavailable',
          };

    return AppListCard(
      label: 'Notifications',
      verticalPadding: Grid.twelve,
      children: [
        AppListRow(
          key: const ValueKey('push-notifications-enabled'),
          icon: LucideIcons.bell,
          title: 'Push notifications',
          subtitle: subtitle,
          subtitleStyle: showSettingsRecovery
              ? context.textTheme.bodySmall?.copyWith(
                  color: context.colors.error,
                )
              : null,
          trailing: Switch.adaptive(
            value: community.pushNotificationsEnabled,
            onChanged: (enabled) => unawaited(
              ref
                  .read(communityListProvider.notifier)
                  .setPushNotificationsEnabled(community.id, enabled),
            ),
          ),
          onTap: () => unawaited(
            ref
                .read(communityListProvider.notifier)
                .setPushNotificationsEnabled(
                  community.id,
                  !community.pushNotificationsEnabled,
                ),
          ),
        ),
        if (showSettingsRecovery)
          AppListRow(
            key: const ValueKey('push-notifications-open-settings'),
            icon: LucideIcons.settings,
            title: 'Open iOS Notification Settings',
            onTap: () => unawaited(
              ref.read(majuPushNotificationSettingsOpenerProvider)(),
            ),
          ),
      ],
    );
  }
}
