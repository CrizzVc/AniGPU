class SourceInfo {
  const SourceInfo({required this.id, required this.name});

  final String id;
  final String name;

  factory SourceInfo.fromJson(Map<String, dynamic> json) {
    return SourceInfo(
      id: json['id'] as String,
      name: json['name'] as String,
    );
  }
}

class LatestItem {
  const LatestItem({
    required this.title,
    required this.url,
    this.episode,
    this.image,
    this.cover,
    this.animeUrl,
  });

  final String title;
  final String url;
  final String? episode;
  final String? image;
  final String? cover;
  final String? animeUrl;

  factory LatestItem.fromJson(Map<String, dynamic> json) {
    return LatestItem(
      title: json['title'] as String? ?? '',
      url: json['url'] as String? ?? '',
      episode: json['episode']?.toString(),
      image: json['image'] as String?,
      cover: json['cover'] as String?,
      animeUrl: json['animeUrl'] as String?,
    );
  }
}

class CardItem {
  const CardItem({
    required this.title,
    required this.url,
    this.image,
    this.animeUrl,
  });

  final String title;
  final String url;
  final String? image;
  final String? animeUrl;

  factory CardItem.fromJson(Map<String, dynamic> json) {
    return CardItem(
      title: json['title'] as String? ?? '',
      url: json['url'] as String? ?? '',
      image: json['image'] as String?,
      animeUrl: json['animeUrl'] as String?,
    );
  }
}

class EpisodeItem {
  const EpisodeItem({
    required this.episode,
    required this.url,
    this.image,
  });

  final String episode;
  final String url;
  final String? image;

  factory EpisodeItem.fromJson(Map<String, dynamic> json) {
    final raw = json['episode'];
    return EpisodeItem(
      episode: raw == null ? '' : raw.toString(),
      url: json['url'] as String? ?? '',
      image: json['image'] as String?,
    );
  }
}

class RelatedItem {
  const RelatedItem({
    required this.title,
    required this.url,
    this.image,
    this.kind,
  });

  final String title;
  final String url;
  final String? image;
  final String? kind;

  factory RelatedItem.fromJson(Map<String, dynamic> json) {
    return RelatedItem(
      title: json['title'] as String? ?? '',
      url: json['url'] as String? ?? '',
      image: json['image'] as String?,
      kind: json['type'] as String?,
    );
  }
}

class AnimeDetails {
  const AnimeDetails({
    required this.title,
    required this.synopsis,
    this.cover,
    this.backdrop,
    this.status,
    this.genres = const [],
    this.related = const [],
    this.episodes = const [],
  });

  final String title;
  final String synopsis;
  final String? cover;
  final String? backdrop;
  final String? status;
  final List<String> genres;
  final List<RelatedItem> related;
  final List<EpisodeItem> episodes;

  factory AnimeDetails.fromJson(Map<String, dynamic> json) {
    return AnimeDetails(
      title: json['title'] as String? ?? '',
      synopsis: json['synopsis'] as String? ?? '',
      cover: json['cover'] as String?,
      backdrop: json['backdrop'] as String?,
      status: json['status'] as String?,
      genres: (json['genres'] as List<dynamic>? ?? [])
          .map((e) => e.toString())
          .toList(),
      related: (json['related'] as List<dynamic>? ?? [])
          .map((e) => RelatedItem.fromJson(e as Map<String, dynamic>))
          .toList(),
      episodes: (json['episodes'] as List<dynamic>? ?? [])
          .map((e) => EpisodeItem.fromJson(e as Map<String, dynamic>))
          .toList(),
    );
  }
}

class ServerItem {
  const ServerItem({
    required this.title,
    this.server,
    this.code,
    this.url,
  });

  final String title;
  final String? server;
  final String? code;
  final String? url;

  String? get embedUrl => code ?? url;

  factory ServerItem.fromJson(Map<String, dynamic> json) {
    return ServerItem(
      title: json['title'] as String? ?? '',
      server: json['server'] as String?,
      code: json['code'] as String?,
      url: json['url'] as String?,
    );
  }
}

class Subtitle {
  const Subtitle({required this.file, required this.label});

  final String file;
  final String label;

  factory Subtitle.fromJson(Map<String, dynamic> json) {
    return Subtitle(
      file: json['file'] as String? ?? '',
      label: json['label'] as String? ?? '',
    );
  }
}

class Extraction {
  const Extraction({
    required this.streamUrl,
    required this.isDirect,
    required this.provider,
    required this.originalUrl,
    this.type,
    this.subtitles = const [],
  });

  final String streamUrl;
  final bool isDirect;
  final String? type;
  final List<Subtitle> subtitles;
  final String provider;
  final String originalUrl;

  factory Extraction.fromJson(Map<String, dynamic> json) {
    return Extraction(
      streamUrl: json['streamUrl'] as String? ?? '',
      isDirect: json['isDirect'] as bool? ?? true,
      type: json['type'] as String?,
      subtitles: (json['subtitles'] as List<dynamic>? ?? [])
          .map((e) => Subtitle.fromJson(e as Map<String, dynamic>))
          .toList(),
      provider: json['provider'] as String? ?? '',
      originalUrl: json['originalUrl'] as String? ?? '',
    );
  }
}

class ApiException implements Exception {
  ApiException(this.message);
  final String message;

  @override
  String toString() => message;
}
