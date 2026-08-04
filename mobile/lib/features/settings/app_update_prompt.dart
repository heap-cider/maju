import 'package:flutter/material.dart';
import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';

import '../../shared/theme/theme.dart';
import 'app_update.dart';

class AppUpdateListener extends HookConsumerWidget {
  const AppUpdateListener({super.key, required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final supported = ref.watch(appUpdateSupportedProvider);
    final check = supported ? ref.watch(appUpdateCheckProvider) : null;
    final update = check?.value;
    final shownVersion = useRef<String?>(null);

    useEffect(() {
      if (update == null || shownVersion.value == update.version) return null;
      shownVersion.value = update.version;
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (!context.mounted) return;
        ScaffoldMessenger.maybeOf(context)?.showSnackBar(
          SnackBar(
            content: Text('Maju v${update.version} is ready.'),
            action: SnackBarAction(
              label: 'Update',
              onPressed: () => showAppUpdateDialog(
                context,
                ref.read(appUpdateServiceProvider),
                update,
              ),
            ),
          ),
        );
      });
      return null;
    }, [update?.version]);

    return child;
  }
}

Future<void> showAppUpdateDialog(
  BuildContext context,
  AppUpdateService service,
  AppUpdateInfo update,
) {
  return showDialog<void>(
    context: context,
    builder: (_) => _AppUpdateDialog(service: service, update: update),
  );
}

class _AppUpdateDialog extends HookWidget {
  const _AppUpdateDialog({required this.service, required this.update});

  final AppUpdateService service;
  final AppUpdateInfo update;

  @override
  Widget build(BuildContext context) {
    final downloading = useState(false);
    final progress = useState<double?>(null);
    final error = useState<String?>(null);

    Future<void> startUpdate() async {
      downloading.value = true;
      progress.value = null;
      error.value = null;
      try {
        await service.downloadAndInstall(
          update,
          onProgress: (value) {
            if (context.mounted) progress.value = value;
          },
        );
        if (context.mounted) Navigator.of(context).pop();
      } on Object catch (exception) {
        if (!context.mounted) return;
        error.value = exception.toString();
        downloading.value = false;
      }
    }

    return AlertDialog(
      title: Text('Maju v${update.version}'),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const Text(
            'Maju will download the new APK, then Android will ask you to '
            'approve the update. Your data and pairing stay in place.',
          ),
          const SizedBox(height: Grid.xs),
          Text(
            'Android may ask you to allow installs from Maju the first time.',
            style: context.textTheme.bodySmall?.copyWith(
              color: context.colors.onSurfaceVariant,
            ),
          ),
          if (downloading.value) ...[
            const SizedBox(height: Grid.twelve),
            LinearProgressIndicator(value: progress.value),
          ],
          if (error.value != null) ...[
            const SizedBox(height: Grid.twelve),
            Text(
              error.value!,
              style: context.textTheme.bodySmall?.copyWith(
                color: context.colors.error,
              ),
            ),
          ],
        ],
      ),
      actions: [
        TextButton(
          onPressed: downloading.value
              ? null
              : () => Navigator.of(context).pop(),
          child: const Text('Later'),
        ),
        FilledButton(
          onPressed: downloading.value ? null : startUpdate,
          child: Text(downloading.value ? 'Downloading…' : 'Download update'),
        ),
      ],
    );
  }
}
