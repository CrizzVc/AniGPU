import 'package:cached_network_image/cached_network_image.dart';
import 'package:flutter/material.dart';

import '../models/models.dart';
import '../state/app_state.dart';
import 'player_screen.dart';

class DetailsScreen extends StatefulWidget {
  const DetailsScreen({
    super.key,
    required this.state,
    required this.title,
    required this.detailsUrl,
    this.poster,
  });

  final AppState state;
  final String title;
  final String detailsUrl;
  final String? poster;

  @override
  State<DetailsScreen> createState() => _DetailsScreenState();
}

class _DetailsScreenState extends State<DetailsScreen> {
  bool _loading = true;
  String? _error;
  AnimeDetails? _details;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      final details = await widget.state.api.details(
        source: widget.state.sourceId,
        url: widget.detailsUrl,
      );
      if (!mounted) return;
      setState(() {
        _details = details;
        _loading = false;
      });
    } on ApiException catch (e) {
      if (!mounted) return;
      setState(() {
        _error = e.message;
        _loading = false;
      });
    }
  }

  Future<void> _playEpisode(EpisodeItem episode) async {
    showDialog<void>(
      context: context,
      barrierDismissible: false,
      builder: (_) => const Center(child: CircularProgressIndicator()),
    );
    try {
      final servers = await widget.state.api.servers(
        source: widget.state.sourceId,
        url: episode.url,
      );
      if (!mounted) return;
      Navigator.of(context).pop();
      if (servers.isEmpty) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('No hay servidores para este episodio.')),
        );
        return;
      }
      final chosen = await showModalBottomSheet<ServerItem>(
        context: context,
        showDragHandle: true,
        builder: (context) {
          return ListView(
            children: [
              const ListTile(title: Text('Elegí un servidor')),
              for (final server in servers)
                ListTile(
                  title: Text(server.title),
                  subtitle: server.server != null ? Text(server.server!) : null,
                  onTap: () => Navigator.pop(context, server),
                ),
            ],
          );
        },
      );
      if (chosen == null || !mounted) return;
      final embed = chosen.embedUrl;
      if (embed == null || embed.isEmpty) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('El servidor no tiene URL de embed.')),
        );
        return;
      }

      showDialog<void>(
        context: context,
        barrierDismissible: false,
        builder: (_) => const Center(child: CircularProgressIndicator()),
      );
      final extraction = await widget.state.api.extract(url: embed);
      if (!mounted) return;
      Navigator.of(context).pop();

      await Navigator.of(context).push(
        MaterialPageRoute(
          builder: (_) => PlayerScreen(
            title: '${_details?.title ?? widget.title} — Ep. ${episode.episode}',
            extraction: extraction,
          ),
        ),
      );
    } on ApiException catch (e) {
      if (!mounted) return;
      Navigator.of(context).pop();
      ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(e.message)));
    }
  }

  @override
  Widget build(BuildContext context) {
    final details = _details;
    return Scaffold(
      appBar: AppBar(title: Text(details?.title ?? widget.title)),
      body: _loading
          ? const Center(child: CircularProgressIndicator())
          : _error != null
              ? Center(
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Text(_error!),
                      const SizedBox(height: 12),
                      FilledButton(onPressed: _load, child: const Text('Reintentar')),
                    ],
                  ),
                )
              : details == null
                  ? const SizedBox.shrink()
                  : ListView(
                      padding: const EdgeInsets.all(16),
                      children: [
                        Row(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            ClipRRect(
                              borderRadius: BorderRadius.circular(12),
                              child: SizedBox(
                                width: 140,
                                height: 200,
                                child: (details.cover ?? widget.poster) == null
                                    ? const ColoredBox(color: Color(0xFF1C1C24))
                                    : CachedNetworkImage(
                                        imageUrl: details.cover ?? widget.poster!,
                                        fit: BoxFit.cover,
                                      ),
                              ),
                            ),
                            const SizedBox(width: 16),
                            Expanded(
                              child: Column(
                                crossAxisAlignment: CrossAxisAlignment.start,
                                children: [
                                  Text(
                                    details.title,
                                    style: Theme.of(context).textTheme.headlineSmall,
                                  ),
                                  if (details.status != null)
                                    Padding(
                                      padding: const EdgeInsets.only(top: 8),
                                      child: Text(details.status!),
                                    ),
                                  const SizedBox(height: 8),
                                  Wrap(
                                    spacing: 8,
                                    runSpacing: 8,
                                    children: [
                                      for (final genre in details.genres)
                                        Chip(label: Text(genre)),
                                    ],
                                  ),
                                ],
                              ),
                            ),
                          ],
                        ),
                        const SizedBox(height: 16),
                        Text(details.synopsis),
                        const SizedBox(height: 24),
                        Text(
                          'Episodios (${details.episodes.length})',
                          style: Theme.of(context).textTheme.titleLarge,
                        ),
                        const SizedBox(height: 8),
                        for (final episode in details.episodes)
                          ListTile(
                            contentPadding: EdgeInsets.zero,
                            title: Text('Episodio ${episode.episode}'),
                            trailing: const Icon(Icons.play_arrow),
                            onTap: () => _playEpisode(episode),
                          ),
                        if (details.related.isNotEmpty) ...[
                          const SizedBox(height: 16),
                          Text(
                            'Relacionados',
                            style: Theme.of(context).textTheme.titleLarge,
                          ),
                          for (final related in details.related)
                            ListTile(
                              contentPadding: EdgeInsets.zero,
                              title: Text(related.title),
                              subtitle: related.kind != null ? Text(related.kind!) : null,
                              onTap: () {
                                Navigator.of(context).pushReplacement(
                                  MaterialPageRoute(
                                    builder: (_) => DetailsScreen(
                                      state: widget.state,
                                      title: related.title,
                                      detailsUrl: related.url,
                                      poster: related.image,
                                    ),
                                  ),
                                );
                              },
                            ),
                        ],
                      ],
                    ),
    );
  }
}
