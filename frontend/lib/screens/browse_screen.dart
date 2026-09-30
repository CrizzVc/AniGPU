import 'package:flutter/material.dart';

import '../models/models.dart';
import '../state/app_state.dart';
import '../widgets/poster_card.dart';
import 'details_screen.dart';

class BrowseScreen extends StatefulWidget {
  const BrowseScreen({super.key, required this.state});

  final AppState state;

  @override
  State<BrowseScreen> createState() => _BrowseScreenState();
}

class _BrowseScreenState extends State<BrowseScreen> {
  bool _loading = true;
  String? _error;
  List<CardItem> _items = const [];
  int _page = 1;
  String? _loadedFor;

  @override
  void initState() {
    super.initState();
    widget.state.addListener(_onState);
    _reload(1);
  }

  @override
  void dispose() {
    widget.state.removeListener(_onState);
    super.dispose();
  }

  void _onState() {
    if (_loadedFor != widget.state.sourceId) {
      _reload(1);
    }
  }

  Future<void> _reload(int page) async {
    final source = widget.state.sourceId;
    setState(() {
      _loading = true;
      _error = null;
      _page = page;
      _loadedFor = source;
    });
    try {
      final items = await widget.state.api.browse(source: source, page: page);
      if (!mounted || _loadedFor != source) return;
      setState(() {
        _items = items;
        _loading = false;
      });
    } on ApiException catch (e) {
      if (!mounted || _loadedFor != source) return;
      setState(() {
        _error = e.message;
        _loading = false;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 8, 16, 0),
          child: Row(
            children: [
              Text('Página $_page', style: Theme.of(context).textTheme.titleMedium),
              const Spacer(),
              IconButton(
                onPressed: _page > 1 && !_loading ? () => _reload(_page - 1) : null,
                icon: const Icon(Icons.chevron_left),
              ),
              IconButton(
                onPressed: !_loading ? () => _reload(_page + 1) : null,
                icon: const Icon(Icons.chevron_right),
              ),
            ],
          ),
        ),
        Expanded(
          child: _loading
              ? const Center(child: CircularProgressIndicator())
              : _error != null
                  ? Center(child: Text(_error!))
                  : GridView.builder(
                      padding: const EdgeInsets.all(16),
                      gridDelegate:
                          const SliverGridDelegateWithMaxCrossAxisExtent(
                        maxCrossAxisExtent: 180,
                        childAspectRatio: 0.58,
                        crossAxisSpacing: 12,
                        mainAxisSpacing: 12,
                      ),
                      itemCount: _items.length,
                      itemBuilder: (context, i) {
                        final item = _items[i];
                        return PosterCard(
                          title: item.title,
                          imageUrl: item.image,
                          onTap: () {
                            Navigator.of(context).push(
                              MaterialPageRoute(
                                builder: (_) => DetailsScreen(
                                  state: widget.state,
                                  title: item.title,
                                  detailsUrl: item.animeUrl ?? item.url,
                                  poster: item.image,
                                ),
                              ),
                            );
                          },
                        );
                      },
                    ),
        ),
      ],
    );
  }
}
