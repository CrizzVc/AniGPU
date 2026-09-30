import 'package:flutter/material.dart';

import '../models/models.dart';
import '../state/app_state.dart';

class SourceSelector extends StatelessWidget {
  const SourceSelector({super.key, required this.state});

  final AppState state;

  @override
  Widget build(BuildContext context) {
    return DropdownButtonHideUnderline(
      child: DropdownButton<String>(
        value: state.sources.any((s) => s.id == state.sourceId)
            ? state.sourceId
            : null,
        hint: const Text('Fuente'),
        items: [
          for (final SourceInfo source in state.sources)
            DropdownMenuItem(
              value: source.id,
              child: Text(source.name),
            ),
        ],
        onChanged: (id) {
          if (id != null) state.setSource(id);
        },
      ),
    );
  }
}
