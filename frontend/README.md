# Frontend Flutter

Cliente de AniGPU que habla con `crates/anigpu-server` (Axum en el puerto **3000** por defecto).

## Requisitos

- [Flutter SDK](https://docs.flutter.dev/get-started/install) en el PATH
- Backend: `cargo run -p anigpu-server`

## Primera vez (generar Android / Windows / web)

Este repo incluye `lib/` y `pubspec.yaml`. Las carpetas de plataforma las crea Flutter:

```powershell
cd frontend
flutter create . --project-name anigpu --org com.anigpu
flutter pub get
```

En Android, para HTTP al emulador, habilitá tráfico en claro (`usesCleartextTraffic`) o usá `--dart-define` con HTTPS.

## Correr

Terminal 1:

```powershell
cargo run -p anigpu-server
```

Terminal 2:

```powershell
cd frontend
flutter run
# o una URL concreta:
flutter run --dart-define=ANIGPU_API=http://127.0.0.1:3000
```

En el emulador Android el cliente usa `http://10.0.2.2:3000` si no pasás `ANIGPU_API`.

## API usada

| Ruta | Uso |
|------|-----|
| `GET /api/sources` | Lista de fuentes |
| `GET /api/latest?source=` | Capítulos recientes |
| `GET /api/browse?source=&page=` | Catálogo |
| `GET /api/search?source=&q=` | Búsqueda |
| `GET /api/anime-details?source=&url=` | Ficha |
| `GET /api/servers?source=&url=` | Servidores del episodio |
| `GET /api/extract?url=` | Stream extraído |

La fuente también se envía en el header `x-source`.
