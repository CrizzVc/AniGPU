import 'package:flutter/material.dart';

import '../models/models.dart';
import '../playback/play_flow.dart';
import '../state/app_state.dart';
import '../util/posters.dart';
import '../util/tmdb.dart';
import '../widgets/remote_image.dart';
import 'details_screen.dart';

class HomeScreen extends StatefulWidget {
  const HomeScreen({super.key, required this.state});

  final AppState state;

  @override
  State<HomeScreen> createState() => _HomeScreenState();
}

class _HomeScreenState extends State<HomeScreen> {
  bool _loading = true;
  String? _error;
  List<LatestItem> _items = const [];
  String? _loadedFor;
  AnimeDetails? _featuredDetails;
  int _featuredIndex = 0;

  LatestItem? get _featured =>
      _items.isEmpty ? null : _items[_featuredIndex.clamp(0, _items.length - 1)];

  @override
  void initState() {
    super.initState();
    widget.state.addListener(_onState);
    _reload();
  }

  @override
  void dispose() {
    widget.state.removeListener(_onState);
    super.dispose();
  }

  void _onState() {
    if (_loadedFor != widget.state.sourceId) {
      _reload();
    }
  }

  Future<void> _reload() async {
    final source = widget.state.sourceId;
    setState(() {
      _loading = true;
      _error = null;
      _loadedFor = source;
      _featuredDetails = null;
      _featuredIndex = 0;
    });
    try {
      final items = await widget.state.api.latest(source: source);
      if (!mounted || _loadedFor != source) return;
      setState(() {
        _items = items;
        _loading = false;
      });
      await _loadFeaturedDetails();
    } on ApiException catch (e) {
      if (!mounted || _loadedFor != source) return;
      setState(() {
        _error = e.message;
        _loading = false;
      });
    }
  }

  Future<void> _loadFeaturedDetails() async {
    final item = _featured;
    final url = item?.animeUrl ?? item?.url;
    if (url == null || url.isEmpty) return;
    final source = widget.state.sourceId;
    try {
      final details = await widget.state.api.details(source: source, url: url);
      if (!mounted || _loadedFor != source) return;
      setState(() => _featuredDetails = details);
    } on ApiException {
      // El hero puede vivir con la portada derivada.
    }
  }

  String? _posterFor(LatestItem item) {
    if (identical(item, _featured) && _featuredDetails?.cover != null) {
      return _featuredDetails!.cover;
    }
    return detailsPoster(cover: item.cover, image: item.image);
  }

  String? get _heroImage {
    final item = _featured;
    if (item == null) return null;
    return completeTmdbUrl(_featuredDetails?.backdrop) ??
        completeTmdbUrl(_featuredDetails?.cover) ??
        _posterFor(item);
  }

  void _openDetails(LatestItem item) {
    Navigator.of(context).push(
      MaterialPageRoute(
        builder: (_) => DetailsScreen(
          state: widget.state,
          title: item.title,
          detailsUrl: item.animeUrl ?? item.url,
          poster: _posterFor(item),
        ),
      ),
    );
  }

  Future<void> _play(LatestItem item) {
    return playEpisodeUrl(
      context: context,
      state: widget.state,
      episodeUrl: item.url,
      title: '${item.title} — ${item.episode ?? ''}'.trim(),
    );
  }

  void _selectFeatured(int index) {
    setState(() {
      _featuredIndex = index;
      _featuredDetails = null;
    });
    _loadFeaturedDetails();
  }

  @override
  Widget build(BuildContext context) {
    if (_loading) {
      return const Center(child: CircularProgressIndicator());
    }
    if (_error != null) {
      return _ErrorView(message: _error!, onRetry: _reload);
    }
    if (_items.isEmpty) {
      return const Center(child: Text('No hay episodios recientes.'));
    }

    final featured = _featured!;

    return RefreshIndicator(
      onRefresh: _reload,
      child: LayoutBuilder(
        builder: (context, constraints) {
          return Stack(
            fit: StackFit.expand,
            children: [
              RemoteImage(url: _heroImage),
              const DecoratedBox(
                decoration: BoxDecoration(
                  gradient: LinearGradient(
                    begin: Alignment.topCenter,
                    end: Alignment.bottomCenter,
                    colors: [
                      Color(0x66000000),
                      Color(0x00000000),
                      Color(0xCC0E0E12),
                      Color(0xFF0E0E12),
                    ],
                    stops: [0, 0.28, 0.62, 1],
                  ),
                ),
              ),
              ListView(
                physics: const AlwaysScrollableScrollPhysics(),
                padding: EdgeInsets.fromLTRB(
                  32,
                  28,
                  32,
                  constraints.maxHeight * 0.02,
                ),
                children: [
                  SizedBox(height: constraints.maxHeight * 0.28),
                  Text(
                    _episodeLabel(featured.episode).toUpperCase(),
                    style: TextStyle(
                      color: Colors.white.withValues(alpha: 0.75),
                      letterSpacing: 2.4,
                      fontSize: 12,
                      fontWeight: FontWeight.w600,
                    ),
                  ),
                  const SizedBox(height: 8),
                  ConstrainedBox(
                    constraints: const BoxConstraints(maxWidth: 720),
                    child: Text(
                      featured.title,
                      style: const TextStyle(
                        fontSize: 42,
                        fontWeight: FontWeight.w700,
                        height: 1.1,
                      ),
                    ),
                  ),
                  const SizedBox(height: 20),
                  Row(
                    children: [
                      FilledButton.icon(
                        onPressed: () => _play(featured),
                        style: FilledButton.styleFrom(
                          backgroundColor: Colors.white,
                          foregroundColor: Colors.black,
                          padding: const EdgeInsets.symmetric(
                            horizontal: 22,
                            vertical: 14,
                          ),
                        ),
                        icon: const Icon(Icons.play_arrow_rounded),
                        label: const Text('Reproducir'),
                      ),
                      const SizedBox(width: 10),
                      IconButton.filledTonal(
                        onPressed: () => _openDetails(featured),
                        icon: const Icon(Icons.more_horiz),
                      ),
                    ],
                  ),
                  SizedBox(height: constraints.maxHeight * 0.08),
                  const Text(
                    'Últimos Episodios',
                    style: TextStyle(fontSize: 18, fontWeight: FontWeight.w600),
                  ),
                  const SizedBox(height: 14),
                  SizedBox(
                    height: 168,
                    child: ListView.separated(
                      scrollDirection: Axis.horizontal,
                      itemCount: _items.length,
                      separatorBuilder: (_, __) => const SizedBox(width: 14),
                      itemBuilder: (context, i) {
                        final item = _items[i];
                        return _LatestCard(
                          title: item.title,
                          episode: _episodeLabel(item.episode),
                          posterUrl: _posterFor(item),
                          selected: i == _featuredIndex,
                          onTap: () {
                            if (i == _featuredIndex) {
                              _openDetails(item);
                            } else {
                              _selectFeatured(i);
                            }
                          },
                          onOpen: () => _openDetails(item),
                        );
                      },
                    ),
                  ),
                ],
              ),
            ],
          );
        },
      ),
    );
  }
}

String _episodeLabel(String? raw) {
  if (raw == null || raw.trim().isEmpty) return 'Episodio';
  final value = raw.trim();
  if (value.toLowerCase().startsWith('episodio')) return value;
  return 'Episodio $value';
}

class _LatestCard extends StatelessWidget {
  const _LatestCard({
    required this.title,
    required this.episode,
    required this.onTap,
    required this.onOpen,
    required this.selected,
    this.posterUrl,
  });

  final String title;
  final String episode;
  final String? posterUrl;
  final bool selected;
  final VoidCallback onTap;
  final VoidCallback onOpen;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: 268,
      child: Material(
        color: Colors.transparent,
        child: InkWell(
          onTap: onTap,
          onLongPress: onOpen,
          borderRadius: BorderRadius.circular(14),
          child: Ink(
            decoration: BoxDecoration(
              borderRadius: BorderRadius.circular(14),
              border: Border.all(
                color: selected ? Colors.white70 : Colors.transparent,
                width: 2,
              ),
            ),
            child: ClipRRect(
              borderRadius: BorderRadius.circular(12),
              child: Stack(
                fit: StackFit.expand,
                children: [
                  RemoteImage(url: posterUrl),
                  const DecoratedBox(
                    decoration: BoxDecoration(
                      gradient: LinearGradient(
                        begin: Alignment.topCenter,
                        end: Alignment.bottomCenter,
                        colors: [Colors.transparent, Color(0xE6000000)],
                      ),
                    ),
                  ),
                  Positioned(
                    left: 12,
                    right: 12,
                    bottom: 10,
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          title,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: const TextStyle(
                            fontWeight: FontWeight.w700,
                            fontSize: 13,
                          ),
                        ),
                        Text(
                          episode,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: TextStyle(
                            color: Colors.white.withValues(alpha: 0.75),
                            fontSize: 12,
                          ),
                        ),
                      ],
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _ErrorView extends StatelessWidget {
  const _ErrorView({required this.message, required this.onRetry});

  final String message;
  final VoidCallback onRetry;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(24),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(message, textAlign: TextAlign.center),
            const SizedBox(height: 16),
            FilledButton(onPressed: onRetry, child: const Text('Reintentar')),
          ],
        ),
      ),
    );
  }
}
