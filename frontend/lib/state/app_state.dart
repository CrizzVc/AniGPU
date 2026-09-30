import 'package:flutter/foundation.dart';

import '../api/anigpu_api.dart';
import '../models/models.dart';

class AppState extends ChangeNotifier {
  AppState({AniGpuApi? api}) : api = api ?? AniGpuApi();

  final AniGpuApi api;

  List<SourceInfo> sources = const [
    SourceInfo(id: 'animeav1', name: 'AnimeAV1'),
    SourceInfo(id: 'jkanime', name: 'JKAnime'),
    SourceInfo(id: 'animeflv', name: 'AnimeFLV'),
    SourceInfo(id: 'animeonlineninja', name: 'AnimeOnlineNinja'),
  ];

  String sourceId = 'animeav1';
  String? error;

  Future<void> loadSources() async {
    try {
      final remote = await api.sources();
      if (remote.isNotEmpty) {
        sources = remote;
        if (!sources.any((s) => s.id == sourceId)) {
          sourceId = sources.first.id;
        }
        error = null;
        notifyListeners();
      }
    } on ApiException catch (e) {
      error = e.message;
      notifyListeners();
    }
  }

  void setSource(String id) {
    if (sourceId == id) return;
    sourceId = id;
    notifyListeners();
  }
}
