import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:http/http.dart' as http;
import 'package:open_filex/open_filex.dart';
import 'package:package_info_plus/package_info_plus.dart';
import 'package:path_provider/path_provider.dart';

const _maxManifestBytes = 64 * 1024;
const _maxApkBytes = 512 * 1024 * 1024;
const _apkContentType = 'application/vnd.android.package-archive';

final _latestManifestUri = Uri.parse(
  'https://github.com/heap-cider/maju/releases/latest/download/latest.json',
);

typedef AppVersionLoader = Future<String> Function();
typedef AppUpdateDirectoryLoader = Future<Directory> Function();
typedef AppApkOpener = Future<OpenResult> Function(String path);

final appUpdateSupportedProvider = Provider<bool>(
  (ref) => Platform.isAndroid && kReleaseMode,
);

final appUpdateHttpClientProvider = Provider<http.Client>((ref) {
  final client = http.Client();
  ref.onDispose(client.close);
  return client;
});

final appUpdateServiceProvider = Provider<AppUpdateService>((ref) {
  return AppUpdateService(
    client: ref.watch(appUpdateHttpClientProvider),
    isSupported: ref.watch(appUpdateSupportedProvider),
    currentVersionLoader: () async =>
        (await PackageInfo.fromPlatform()).version,
    directoryLoader: getTemporaryDirectory,
    apkOpener: (path) => OpenFilex.open(path, type: _apkContentType),
  );
});

final appUpdateCheckProvider = FutureProvider<AppUpdateInfo?>((ref) {
  return ref.watch(appUpdateServiceProvider).checkForUpdate();
});

class AppUpdateInfo {
  const AppUpdateInfo({
    required this.currentVersion,
    required this.version,
    required this.apkUri,
  });

  final String currentVersion;
  final String version;
  final Uri apkUri;
}

class AppUpdateException implements Exception {
  const AppUpdateException(this.message);

  final String message;

  @override
  String toString() => message;
}

class AppUpdateService {
  AppUpdateService({
    required http.Client client,
    required this.isSupported,
    required AppVersionLoader currentVersionLoader,
    required AppUpdateDirectoryLoader directoryLoader,
    required AppApkOpener apkOpener,
    Uri? manifestUri,
    this.requestTimeout = const Duration(seconds: 10),
    this.downloadIdleTimeout = const Duration(seconds: 30),
  }) : _client = client,
       _currentVersionLoader = currentVersionLoader,
       _directoryLoader = directoryLoader,
       _apkOpener = apkOpener,
       _manifestUri = manifestUri ?? _latestManifestUri;

  final http.Client _client;
  final AppVersionLoader _currentVersionLoader;
  final AppUpdateDirectoryLoader _directoryLoader;
  final AppApkOpener _apkOpener;
  final Uri _manifestUri;

  final bool isSupported;
  final Duration requestTimeout;
  final Duration downloadIdleTimeout;

  Future<AppUpdateInfo?> checkForUpdate() async {
    if (!isSupported) return null;

    final response = await _client
        .get(_manifestUri, headers: const {'Accept': 'application/json'})
        .timeout(requestTimeout);
    if (response.statusCode != HttpStatus.ok) {
      throw AppUpdateException('Update check failed (${response.statusCode}).');
    }
    if (response.bodyBytes.length > _maxManifestBytes) {
      throw const AppUpdateException('Update information was too large.');
    }

    final Object? decoded;
    try {
      decoded = jsonDecode(utf8.decode(response.bodyBytes));
    } on FormatException {
      throw const AppUpdateException('Update information was invalid.');
    }
    if (decoded is! Map<String, dynamic> || decoded['version'] is! String) {
      throw const AppUpdateException('Update information had no version.');
    }

    final latest = AppVersion.parse(decoded['version'] as String);
    final currentText = await _currentVersionLoader();
    final current = AppVersion.parse(currentText);
    if (latest.compareTo(current) <= 0) return null;

    final version = latest.toString();
    return AppUpdateInfo(
      currentVersion: current.toString(),
      version: version,
      apkUri: Uri.parse(
        'https://github.com/heap-cider/maju/releases/download/'
        'v$version/Maju-$version-android.apk',
      ),
    );
  }

  Future<void> downloadAndInstall(
    AppUpdateInfo update, {
    void Function(double? progress)? onProgress,
  }) async {
    if (!isSupported) {
      throw const AppUpdateException(
        'APK updates are only available on Android.',
      );
    }

    final request = http.Request('GET', update.apkUri);
    final response = await _client.send(request).timeout(requestTimeout);
    if (response.statusCode != HttpStatus.ok) {
      throw AppUpdateException(
        'Update download failed (${response.statusCode}).',
      );
    }
    final expectedBytes = response.contentLength;
    if (expectedBytes != null && expectedBytes > _maxApkBytes) {
      throw const AppUpdateException('The update file was too large.');
    }

    final directory = await _directoryLoader();
    await directory.create(recursive: true);
    final apk = File(
      '${directory.path}${Platform.pathSeparator}Maju-update.apk',
    );
    final output = await apk.open(mode: FileMode.write);
    var receivedBytes = 0;
    var complete = false;
    try {
      await for (final chunk in response.stream.timeout(downloadIdleTimeout)) {
        receivedBytes += chunk.length;
        if (receivedBytes > _maxApkBytes) {
          throw const AppUpdateException('The update file was too large.');
        }
        await output.writeFrom(chunk);
        onProgress?.call(
          expectedBytes == null || expectedBytes <= 0
              ? null
              : receivedBytes / expectedBytes,
        );
      }
      if (expectedBytes != null && receivedBytes != expectedBytes) {
        throw const AppUpdateException('The update download was incomplete.');
      }
      complete = true;
    } finally {
      await output.close();
      if (!complete && await apk.exists()) {
        await apk.delete();
      }
    }

    final result = await _apkOpener(apk.path);
    if (result.type != ResultType.done) {
      throw AppUpdateException(
        result.type == ResultType.permissionDenied
            ? 'Android did not allow Maju to open the installer.'
            : 'The Android installer could not be opened.',
      );
    }
  }
}

class AppVersion implements Comparable<AppVersion> {
  const AppVersion(this.major, this.minor, this.patch);

  factory AppVersion.parse(String value) {
    final match = RegExp(
      r'^v?(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:[-+].*)?$',
    ).firstMatch(value.trim());
    if (match == null) {
      throw const AppUpdateException('The app version was invalid.');
    }
    return AppVersion(
      int.parse(match.group(1)!),
      int.parse(match.group(2)!),
      int.parse(match.group(3)!),
    );
  }

  final int major;
  final int minor;
  final int patch;

  @override
  int compareTo(AppVersion other) {
    final majorOrder = major.compareTo(other.major);
    if (majorOrder != 0) return majorOrder;
    final minorOrder = minor.compareTo(other.minor);
    if (minorOrder != 0) return minorOrder;
    return patch.compareTo(other.patch);
  }

  @override
  String toString() => '$major.$minor.$patch';
}
