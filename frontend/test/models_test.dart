import 'package:flutter_test/flutter_test.dart';

import 'package:anigpu/models/models.dart';
import 'package:anigpu/util/posters.dart';

void main() {
  test('parsea un LatestItem camelCase del backend', () {
    final item = LatestItem.fromJson({
      'title': 'Demo',
      'url': 'https://example.test/ep/1',
      'animeUrl': 'https://example.test/anime',
      'episode': '12',
    });
    expect(item.title, 'Demo');
    expect(item.animeUrl, 'https://example.test/anime');
    expect(item.episode, '12');
  });

  test('EpisodeNum untagged llega como int o string', () {
    expect(EpisodeItem.fromJson({'episode': 3, 'url': 'u'}).episode, '3');
    expect(EpisodeItem.fromJson({'episode': 'OVA', 'url': 'u'}).episode, 'OVA');
  });

  test('deriva la portada de ficha desde la miniatura', () {
    expect(
      coverFromThumb('https://cdn.animeav1.com/thumbnails/4412.jpg'),
      'https://cdn.animeav1.com/covers/4412.jpg',
    );
    expect(
      detailsPoster(
        cover: 'https://cdn.animeav1.com/thumbnails/4412.jpg',
        image: 'https://cdn.animeav1.com/thumbnails/4412.jpg',
      ),
      'https://cdn.animeav1.com/covers/4412.jpg',
    );
  });
}
