# AniGPU
La evolucion natural de Tsukuyomi. Reproduce anime en una interfaz moderna, intuitiva y fluida.

El cliente gráfico está en [`frontend/`](frontend/) (Flutter) y se conecta al API de [`crates/anigpu-server`](crates/anigpu-server). Levantá el servidor (`cargo run -p anigpu-server`) y después `flutter run` dentro de `frontend`. Detalle en [frontend/README.md](frontend/README.md).
