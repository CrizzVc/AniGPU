/// Completa una ruta de imagen de TMDB convirtiéndola en una URL absoluta.
String? completeTmdbUrl(String? path) {
  if (path == null || path.isEmpty) return null;
  if (path.startsWith('http')) return path;

  // TMDB images usually start with /
  final cleanPath = path.startsWith('/') ? path : '/$path';
  return 'https://image.tmdb.org/t/p/original$cleanPath';
}
