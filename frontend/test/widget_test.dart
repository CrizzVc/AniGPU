import 'package:flutter_test/flutter_test.dart';

import 'package:anigpu/main.dart';

void main() {
  testWidgets('muestra la shell de AniGPU', (tester) async {
    await tester.pumpWidget(const AniGpuApp());
    expect(find.text('AniGPU'), findsOneWidget);
    expect(find.text('Inicio'), findsOneWidget);
  });
}
