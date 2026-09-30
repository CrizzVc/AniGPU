import 'package:flutter/material.dart';
import 'package:url_launcher/url_launcher.dart';
import 'package:video_player/video_player.dart';

import '../models/models.dart';

class PlayerScreen extends StatefulWidget {
  const PlayerScreen({
    super.key,
    required this.title,
    required this.extraction,
  });

  final String title;
  final Extraction extraction;

  @override
  State<PlayerScreen> createState() => _PlayerScreenState();
}

class _PlayerScreenState extends State<PlayerScreen> {
  VideoPlayerController? _controller;
  String? _error;
  bool _ready = false;

  @override
  void initState() {
    super.initState();
    _init();
  }

  Future<void> _init() async {
    final url = widget.extraction.streamUrl;
    if (url.isEmpty) {
      setState(() => _error = 'El extractor no devolvió una URL.');
      return;
    }
    if (!widget.extraction.isDirect) {
      return;
    }
    final controller = VideoPlayerController.networkUrl(Uri.parse(url));
    try {
      await controller.initialize();
      await controller.play();
      if (!mounted) {
        await controller.dispose();
        return;
      }
      setState(() {
        _controller = controller;
        _ready = true;
      });
    } catch (e) {
      await controller.dispose();
      if (!mounted) return;
      setState(() => _error = 'No se pudo reproducir el stream directo.');
    }
  }

  Future<void> _openExternal() async {
    final uri = Uri.tryParse(widget.extraction.streamUrl);
    if (uri == null) return;
    await launchUrl(uri, mode: LaunchMode.externalApplication);
  }

  @override
  void dispose() {
    _controller?.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final extraction = widget.extraction;
    final controller = _controller;

    return Scaffold(
      appBar: AppBar(title: Text(widget.title)),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          Text('Proveedor: ${extraction.provider}'),
          const SizedBox(height: 8),
          if (_error != null) Text(_error!),
          if (!extraction.isDirect)
            const Text(
              'Este enlace no es un stream directo. Abrilo en el navegador.',
            ),
          const SizedBox(height: 12),
          if (_ready && controller != null)
            AspectRatio(
              aspectRatio: controller.value.aspectRatio == 0
                  ? 16 / 9
                  : controller.value.aspectRatio,
              child: Stack(
                alignment: Alignment.bottomCenter,
                children: [
                  VideoPlayer(controller),
                  VideoProgressIndicator(controller, allowScrubbing: true),
                  Align(
                    alignment: Alignment.center,
                    child: IconButton(
                      iconSize: 48,
                      onPressed: () {
                        setState(() {
                          controller.value.isPlaying
                              ? controller.pause()
                              : controller.play();
                        });
                      },
                      icon: Icon(
                        controller.value.isPlaying
                            ? Icons.pause_circle
                            : Icons.play_circle,
                      ),
                    ),
                  ),
                ],
              ),
            )
          else if (extraction.isDirect && _error == null)
            const Center(
              child: Padding(
                padding: EdgeInsets.all(24),
                child: CircularProgressIndicator(),
              ),
            ),
          const SizedBox(height: 16),
          FilledButton.icon(
            onPressed: _openExternal,
            icon: const Icon(Icons.open_in_new),
            label: const Text('Abrir URL del stream'),
          ),
          if (extraction.subtitles.isNotEmpty) ...[
            const SizedBox(height: 16),
            const Text('Subtítulos'),
            for (final sub in extraction.subtitles)
              ListTile(
                contentPadding: EdgeInsets.zero,
                title: Text(sub.label),
                subtitle: Text(sub.file, maxLines: 1, overflow: TextOverflow.ellipsis),
              ),
          ],
        ],
      ),
    );
  }
}
