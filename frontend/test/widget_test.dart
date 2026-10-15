import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:anigpu/main.dart';

void main() {
  testWidgets('muestra la shell de AniGPU', (tester) async {
    await tester.pumpWidget(const AniGpuApp());
    expect(find.byType(NavigationRail), findsOneWidget);
  });
}
