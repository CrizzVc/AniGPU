import 'dart:convert';

import 'package:http/http.dart' as http;

import '../config.dart';
import '../models/models.dart';

class AniGpuApi {
  AniGpuApi({http.Client? client, String? baseUrl})
      : _client = client ?? http.Client(),
        baseUrl = baseUrl ?? AppConfig.apiBaseUrl;

  final http.Client _client;
  final String baseUrl;

  Future<List<SourceInfo>> sources() async {
    final json = await _get('/api/sources');
    return _list(json['data']).map(SourceInfo.fromJson).toList();
  }

  Future<List<LatestItem>> latest({required String source}) async {
    final json = await _get('/api/latest', source: source);
    return _list(json['data']).map(LatestItem.fromJson).toList();
  }

  Future<List<CardItem>> browse({required String source, int page = 1}) async {
    final json = await _get(
      '/api/browse',
      source: source,
      extra: {'page': '$page'},
    );
    return _list(json['data']).map(CardItem.fromJson).toList();
  }

  Future<List<CardItem>> search({
    required String source,
    required String query,
  }) async {
    final json = await _get(
      '/api/search',
      source: source,
      extra: {'q': query},
    );
    return _list(json['data']).map(CardItem.fromJson).toList();
  }

  Future<AnimeDetails> details({
    required String source,
    required String url,
  }) async {
    final json = await _get(
      '/api/anime-details',
      source: source,
      extra: {'url': url},
    );
    return AnimeDetails.fromJson(json['data'] as Map<String, dynamic>);
  }

  Future<List<ServerItem>> servers({
    required String source,
    required String url,
  }) async {
    final json = await _get(
      '/api/servers',
      source: source,
      extra: {'url': url},
    );
    return _list(json['servers']).map(ServerItem.fromJson).toList();
  }

  Future<Extraction> extract({required String url}) async {
    final json = await _get('/api/extract', extra: {'url': url});
    return Extraction.fromJson(json);
  }

  Future<Map<String, dynamic>> _get(
    String path, {
    String? source,
    Map<String, String>? extra,
  }) async {
    final params = <String, String>{
      if (source != null && source.isNotEmpty) 'source': source,
      ...?extra,
    };
    final uri = Uri.parse('$baseUrl$path').replace(
      queryParameters: params.isEmpty ? null : params,
    );
    final headers = <String, String>{
      if (source != null && source.isNotEmpty) 'x-source': source,
    };

    late http.Response response;
    try {
      response = await _client.get(uri, headers: headers);
    } catch (e) {
      throw ApiException(
        'No se pudo conectar con $baseUrl. ¿Está corriendo anigpu-server?',
      );
    }

    Map<String, dynamic> body;
    try {
      body = jsonDecode(response.body) as Map<String, dynamic>;
    } catch (_) {
      throw ApiException('Respuesta inválida (${response.statusCode}).');
    }

    if (response.statusCode >= 400 || body['success'] == false) {
      throw ApiException(
        body['error'] as String? ?? 'Error HTTP ${response.statusCode}',
      );
    }
    return body;
  }

  List<Map<String, dynamic>> _list(dynamic value) {
    if (value is! List) return const [];
    return value.whereType<Map<String, dynamic>>().toList();
  }
}
