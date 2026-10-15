import 'dart:io';
import 'dart:typed_data';
import 'package:flutter/material.dart';

class RemoteImage extends StatefulWidget {
  const RemoteImage({super.key, required this.url, this.fit = BoxFit.cover});

  final String? url;
  final BoxFit fit;

  @override
  State<RemoteImage> createState() => _RemoteImageState();
}

class _RemoteImageState extends State<RemoteImage> {
  Uint8List? _bytes;
  bool _hasError = false;

  @override
  void initState() {
    super.initState();
    _loadImage();
  }

  @override
  void didUpdateWidget(RemoteImage oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.url != widget.url) {
      _loadImage();
    }
  }

  Future<void> _loadImage() async {
    if (widget.url == null || widget.url!.isEmpty) {
      setState(() {
        _bytes = null;
        _hasError = false;
      });
      return;
    }

    try {
      final client = HttpClient();
      final request = await client.getUrl(Uri.parse(widget.url!));
      request.headers.set('User-Agent', 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36');

      final response = await request.close();
      if (response.statusCode == 200) {
        final List<int> bytes = [];
        await for (final chunk in response) {
          bytes.addAll(chunk);
        }
        if (mounted) {
          setState(() {
            _bytes = Uint8List.fromList(bytes);
            _hasError = false;
          });
        }
      } else {
        debugPrint('Image Error: ${response.statusCode} for ${widget.url}');
        if (mounted) {
          setState(() => _hasError = true);
        }
      }
    } catch (e) {
      debugPrint('Image Exception: $e');
      if (mounted) {
        setState(() => _hasError = true);
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    if (widget.url == null || widget.url!.isEmpty) {
      return const ColoredBox(
        color: Color(0xFF16161C),
        child: Center(child: Icon(Icons.movie_outlined, color: Colors.white24, size: 48)),
      );
    }

    if (_hasError) {
      return ColoredBox(
        color: const Color(0xFF16161C),
        child: Center(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              const Icon(Icons.broken_image_outlined, color: Colors.white24),
              Text('Error loading image', style: TextStyle(color: Colors.white.withOpacity(0.3), fontSize: 10)),
            ],
          ),
        ),
      );
    }

    if (_bytes == null) {
      return const ColoredBox(color: Color(0xFF16161C));
    }

    return Image.memory(
      _bytes!,
      fit: widget.fit,
      width: double.infinity,
      height: double.infinity,
    );
  }
}
