import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:http/testing.dart' as http_testing;
import 'package:maju/features/settings/app_update.dart';
import 'package:maju/features/settings/app_update_prompt.dart';
import 'package:open_filex/open_filex.dart';

void main() {
  testWidgets('announces an update and opens the in-app update dialog', (
    tester,
  ) async {
    final update = AppUpdateInfo(
      currentVersion: '0.2.7',
      version: '0.2.8',
      apkUri: Uri.parse('https://example.test/Maju-0.2.8-android.apk'),
    );
    final service = AppUpdateService(
      client: http_testing.MockClient((_) async => throw StateError('unused')),
      isSupported: true,
      currentVersionLoader: () async => '0.2.7',
      directoryLoader: Directory.systemTemp.createTemp,
      apkOpener: (_) async => OpenResult(),
    );

    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          appUpdateSupportedProvider.overrideWithValue(true),
          appUpdateServiceProvider.overrideWithValue(service),
          appUpdateCheckProvider.overrideWith((ref) async => update),
        ],
        child: const MaterialApp(
          home: AppUpdateListener(child: Scaffold(body: Text('Maju home'))),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('Maju v0.2.8 is ready.'), findsOneWidget);
    await tester.tap(find.text('Update'));
    await tester.pumpAndSettle();

    expect(find.text('Maju v0.2.8'), findsOneWidget);
    expect(find.text('Download update'), findsOneWidget);
    expect(
      find.textContaining('Your data and pairing stay in place.'),
      findsOneWidget,
    );
  });
}
