import 'package:flutter/material.dart';

import '../models/models.dart';
import '../screens/player_screen.dart';
import '../state/app_state.dart';

Future<void> playEpisodeUrl({
  required BuildContext context,
  required AppState state,
  required String episodeUrl,
  required String title,
}) async {
  var dialogOpen = false;

  void showBusy() {
    dialogOpen = true;
    showDialog<void>(
      context: context,
      barrierDismissible: false,
      builder: (_) => const Center(child: CircularProgressIndicator()),
    );
  }

  void hideBusy() {
    if (!dialogOpen) return;
    dialogOpen = false;
    Navigator.of(context).pop();
  }

  showBusy();
  try {
    final servers = await state.api.servers(
      source: state.sourceId,
      url: episodeUrl,
    );
    if (!context.mounted) return;
    hideBusy();
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
    if (chosen == null || !context.mounted) return;
    final embed = chosen.embedUrl;
    if (embed == null || embed.isEmpty) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('El servidor no tiene URL de embed.')),
      );
      return;
    }

    showBusy();
    final extraction = await state.api.extract(url: embed);
    if (!context.mounted) return;
    hideBusy();

    await Navigator.of(context).push(
      MaterialPageRoute(
        builder: (_) => PlayerScreen(title: title, extraction: extraction),
      ),
    );
  } on ApiException catch (e) {
    if (!context.mounted) return;
    hideBusy();
    ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(e.message)));
  }
}
