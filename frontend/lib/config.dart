import 'dart:io' show Platform;

import 'package:flutter/foundation.dart';

/// URL del servidor Axum (`crates/anigpu-server`).
///
/// Override: `--dart-define=ANIGPU_API=http://192.168.1.10:3000`
class AppConfig {
  static const _fromEnv = String.fromEnvironment('ANIGPU_API');

  static String get apiBaseUrl {
    if (_fromEnv.isNotEmpty) return _fromEnv;
    if (kIsWeb) return 'http://127.0.0.1:3000';
    if (!kIsWeb && Platform.isAndroid) return 'http://10.0.2.2:3000';
    return 'http://127.0.0.1:3000';
  }
}
