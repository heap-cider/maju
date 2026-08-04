import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart' as http_testing;
import 'package:maju/features/settings/app_update.dart';
import 'package:open_filex/open_filex.dart';

void main() {
  group('AppVersion', () {
    test('compares semantic versions numerically', () {
      expect(
        AppVersion.parse('0.2.10').compareTo(AppVersion.parse('0.2.9')),
        greaterThan(0),
      );
      expect(
        AppVersion.parse('v1.0.0+12').compareTo(AppVersion.parse('1.0.0')),
        0,
      );
    });

    test('rejects malformed versions', () {
      expect(
        () => AppVersion.parse('../bad'),
        throwsA(isA<AppUpdateException>()),
      );
    });
  });

  test('finds a newer GitHub release and builds its APK URL', () async {
    final service = _service(
      client: http_testing.MockClient((request) async {
        expect(request.url.path, '/latest.json');
        return http.Response(jsonEncode({'version': '0.2.8'}), 200);
      }),
      currentVersion: '0.2.7',
    );

    final update = await service.checkForUpdate();

    expect(update?.currentVersion, '0.2.7');
    expect(update?.version, '0.2.8');
    expect(
      update?.apkUri.toString(),
      'https://github.com/heap-cider/maju/releases/download/'
      'v0.2.8/Maju-0.2.8-android.apk',
    );
  });

  test('returns no update for the installed or an older release', () async {
    for (final latest in ['0.2.7', '0.2.6']) {
      final service = _service(
        client: http_testing.MockClient(
          (_) async => http.Response(jsonEncode({'version': latest}), 200),
        ),
        currentVersion: '0.2.7',
      );
      expect(await service.checkForUpdate(), isNull);
    }
  });

  test('downloads the APK before opening Android installer', () async {
    final directory = await Directory.systemTemp.createTemp(
      'maju-update-test-',
    );
    addTearDown(() => directory.delete(recursive: true));
    const bytes = <int>[1, 2, 3, 4, 5];
    String? openedPath;
    final progress = <double?>[];
    final service = AppUpdateService(
      client: http_testing.MockClient((request) async {
        expect(request.url.path, endsWith('/Maju-0.2.8-android.apk'));
        return http.Response.bytes(bytes, 200);
      }),
      isSupported: true,
      currentVersionLoader: () async => '0.2.7',
      directoryLoader: () async => directory,
      apkOpener: (path) async {
        openedPath = path;
        return OpenResult();
      },
      manifestUri: Uri.parse('https://example.test/latest.json'),
    );
    final update = AppUpdateInfo(
      currentVersion: '0.2.7',
      version: '0.2.8',
      apkUri: Uri.parse(
        'https://github.com/heap-cider/maju/releases/download/'
        'v0.2.8/Maju-0.2.8-android.apk',
      ),
    );

    await service.downloadAndInstall(update, onProgress: progress.add);

    expect(openedPath, isNotNull);
    expect(await File(openedPath!).readAsBytes(), bytes);
    expect(progress.last, 1);
  });

  test('does no network work on unsupported platforms', () async {
    var requested = false;
    final service = _service(
      client: http_testing.MockClient((_) async {
        requested = true;
        return http.Response('{}', 200);
      }),
      currentVersion: '0.2.7',
      isSupported: false,
    );

    expect(await service.checkForUpdate(), isNull);
    expect(requested, isFalse);
  });
}

AppUpdateService _service({
  required http.Client client,
  required String currentVersion,
  bool isSupported = true,
}) {
  return AppUpdateService(
    client: client,
    isSupported: isSupported,
    currentVersionLoader: () async => currentVersion,
    directoryLoader: Directory.systemTemp.createTemp,
    apkOpener: (_) async => OpenResult(),
    manifestUri: Uri.parse('https://example.test/latest.json'),
  );
}
