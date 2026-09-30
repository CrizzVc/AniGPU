import 'package:flutter/material.dart';

import 'screens/browse_screen.dart';
import 'screens/home_screen.dart';
import 'screens/search_screen.dart';
import 'state/app_state.dart';
import 'widgets/source_selector.dart';

void main() {
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
      appBar: AppBar(
        title: const Text('AniGPU'),
        actions: [
          Padding(
            padding: const EdgeInsets.only(right: 16),
            child: SourceSelector(state: widget.state),
          ),
        ],
      ),
      body: IndexedStack(index: _index, children: pages),
      bottomNavigationBar: NavigationBar(
        selectedIndex: _index,
        onDestinationSelected: (i) => setState(() => _index = i),
        destinations: const [
          NavigationDestination(
            icon: Icon(Icons.home_outlined),
            selectedIcon: Icon(Icons.home),
            label: 'Inicio',
          ),
          NavigationDestination(
            icon: Icon(Icons.grid_view_outlined),
            selectedIcon: Icon(Icons.grid_view),
            label: 'Catálogo',
          ),
          NavigationDestination(
            icon: Icon(Icons.search_outlined),
            selectedIcon: Icon(Icons.search),
            label: 'Buscar',
          ),
        ],
      ),
    );
  }
}
