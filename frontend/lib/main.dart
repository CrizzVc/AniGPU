import 'package:flutter/material.dart';

import 'util/http_client.dart';
import 'screens/browse_screen.dart';
import 'screens/home_screen.dart';
import 'screens/search_screen.dart';
import 'state/app_state.dart';

void main() {
  setupHttpOverrides();
  runApp(const AniGpuApp());
}

class AniGpuApp extends StatefulWidget {
  const AniGpuApp({super.key});

  @override
  State<AniGpuApp> createState() => _AniGpuAppState();
}

class _AniGpuAppState extends State<AniGpuApp> {
  late final AppState _state;

  @override
  void initState() {
    super.initState();
    _state = AppState();
    _state.addListener(_onState);
    _state.loadSources();
  }

  void _onState() => setState(() {});

  @override
  void dispose() {
    _state.removeListener(_onState);
    _state.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'AniGPU',
      debugShowCheckedModeBanner: false,
      theme: ThemeData(
        brightness: Brightness.dark,
        colorScheme: ColorScheme.fromSeed(
          seedColor: const Color(0xFF7C5CFF),
          brightness: Brightness.dark,
        ),
        scaffoldBackgroundColor: const Color(0xFF0E0E12),
        useMaterial3: true,
      ),
      home: Shell(state: _state),
    );
  }
}

class Shell extends StatefulWidget {
  const Shell({super.key, required this.state});

  final AppState state;

  @override
  State<Shell> createState() => _ShellState();
}

class _ShellState extends State<Shell> {
  int _index = 0;

  @override
  Widget build(BuildContext context) {
    final pages = [
      HomeScreen(state: widget.state),
      BrowseScreen(state: widget.state),
      SearchScreen(state: widget.state),
    ];

    return Scaffold(
      body: Row(
        children: [
          NavigationRail(
            backgroundColor: const Color(0xFF0A0A0E),
            selectedIndex: _index,
            onDestinationSelected: (i) => setState(() => _index = i),
            labelType: NavigationRailLabelType.none,
            leading: Padding(
              padding: const EdgeInsets.only(bottom: 12, top: 8),
              child: PopupMenuButton<String>(
                tooltip: 'Fuente',
                initialValue: widget.state.sourceId,
                onSelected: widget.state.setSource,
                itemBuilder: (context) => [
                  for (final source in widget.state.sources)
                    PopupMenuItem(
                      value: source.id,
                      child: Text(source.name),
                    ),
                ],
                child: const CircleAvatar(
                  radius: 16,
                  backgroundColor: Color(0xFF1C1C24),
                  child: Icon(Icons.public, size: 18),
                ),
              ),
            ),
            trailing: Expanded(
              child: Align(
                alignment: Alignment.bottomCenter,
                child: Padding(
                  padding: const EdgeInsets.only(bottom: 16),
                  child: Icon(
                    Icons.person_outline,
                    color: Colors.white.withValues(alpha: 0.55),
                  ),
                ),
              ),
            ),
            destinations: const [
              NavigationRailDestination(
                icon: Icon(Icons.home_outlined),
                selectedIcon: Icon(Icons.home),
                label: Text('Inicio'),
              ),
              NavigationRailDestination(
                icon: Icon(Icons.grid_view_outlined),
                selectedIcon: Icon(Icons.grid_view),
                label: Text('Catálogo'),
              ),
              NavigationRailDestination(
                icon: Icon(Icons.search_outlined),
                selectedIcon: Icon(Icons.search),
                label: Text('Buscar'),
              ),
            ],
          ),
          const VerticalDivider(width: 1, color: Color(0xFF1C1C24)),
          Expanded(
            child: IndexedStack(index: _index, children: pages),
          ),
        ],
      ),
    );
  }
}
