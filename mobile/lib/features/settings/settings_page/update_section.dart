part of '../settings_page.dart';

class _AppUpdateSection extends ConsumerWidget {
  const _AppUpdateSection();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    if (!ref.watch(appUpdateSupportedProvider)) {
      return const SizedBox.shrink();
    }

    final check = ref.watch(appUpdateCheckProvider);
    final update = check.value;
    final subtitle = switch (check) {
      AsyncLoading() => 'Checking for a new version…',
      AsyncError() => 'Could not check. Tap to try again.',
      AsyncData(value: null) => 'Maju is up to date.',
      AsyncData(value: AppUpdateInfo update) => 'v${update.version} is ready.',
    };

    return AppListCard(
      label: 'Updates',
      children: [
        AppListRow(
          icon: LucideIcons.download,
          title: 'App update',
          subtitle: subtitle,
          trailing: check.isLoading
              ? const SizedBox.square(
                  dimension: 18,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              : const _RowChevron(),
          onTap: check.isLoading
              ? null
              : () {
                  if (update == null) {
                    ref.invalidate(appUpdateCheckProvider);
                    return;
                  }
                  showAppUpdateDialog(
                    context,
                    ref.read(appUpdateServiceProvider),
                    update,
                  );
                },
        ),
      ],
    );
  }
}
