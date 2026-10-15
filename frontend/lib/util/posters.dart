/// Portada de ficha a partir de la miniatura de capítulo (mismo truco que la app egui).
String? coverFromThumb(String? url) {
  if (url == null || url.isEmpty) return null;
  const marker = '/thumbnails/';
  final i = url.lastIndexOf(marker);
  if (i < 0) return null;
  final base = url.substring(0, i);
  final file = url.substring(i + marker.length);
  final id = file.split('.').first;
  if (id.isEmpty) return null;
  return '$base/covers/$id.jpg';
}

/// Misma imagen que la ficha de detalles, no el still del episodio.
String? detailsPoster({String? cover, String? image}) {
  return coverFromThumb(image) ??
      coverFromThumb(cover) ??
      cover ??
      image;
}
