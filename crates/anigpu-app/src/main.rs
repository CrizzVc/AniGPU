use anigpu_core::models::{AnimeDetails, CardItem, EpisodeItem, LatestItem};
use anigpu_core::sources::get_source;
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use tokio::runtime::Runtime;

fn main() -> eframe::Result<()> {
    let rt = Arc::new(Runtime::new().expect("Failed to create Tokio runtime"));

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([850.0, 550.0])
            .with_maximized(true)
            .with_title("AniGPU"),
        ..Default::default()
    };

    eframe::run_native(
        "AniGPU",
        options,
        Box::new(move |cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            let mut visuals = egui::Visuals::dark();
            visuals.window_fill = egui::Color32::from_rgb(14, 14, 18);
            visuals.panel_fill = egui::Color32::from_rgb(14, 14, 18);
            cc.egui_ctx.set_visuals(visuals);

            let mut app = AniGpuApp::new(rt);
            app.load_latest(cc.egui_ctx.clone());
            Ok(Box::new(app))
        }),
    )
}

// ── Navigation ──────────────────────────────────────────────────────────────

#[derive(Clone)]
#[allow(dead_code)]
enum Screen {
    Home,
    Search,
    Detail {
        /// The URL used to fetch details (anime page URL)
        anime_url: String,
        /// The LatestItem that triggered this detail view (for fallback data)
        origin_item: LatestItem,
    },
}

// ── Detail sub-tab ──────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum DetailTab {
    Info,
    Episodes,
    Related,
}

// ── Progreso de visualización ────────────────────────────────────────────────

/// Episodio empezado y no terminado.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CurrentEpisode {
    /// Número de episodio (EpisodeNum::as_i64, o índice + 1 como respaldo)
    ep: i64,
    /// Posición dentro del episodio: 0.0 ..= 1.0
    #[serde(default)]
    position: f32,
}

/// Historial de un anime: qué capítulos se vieron completos y cuál quedó a medias.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct WatchProgress {
    /// Capítulos vistos por completo
    #[serde(default)]
    completed: Vec<i64>,
    /// Capítulo empezado sin terminar
    #[serde(default)]
    current: Option<CurrentEpisode>,
}

/// La portada se deriva del miniaturizado: en animeav1 el id del anime es el
/// mismo en `thumbnails/` y `covers/`, así que no hace falta pedir nada.
fn cover_from_thumb(url: &str) -> Option<String> {
    let (base, file) = url.rsplit_once("/thumbnails/")?;
    let id = file.split('.').next()?;
    if id.is_empty() {
        return None;
    }
    Some(format!("{base}/covers/{id}.jpg"))
}

/// Fila de la vista de búsqueda (sugerencias de la izquierda y grid de la derecha).
#[derive(Clone)]
struct SearchRow {
    title: String,
    url: String,
    anime_url: Option<String>,
    /// Portada mostrada en el grid (la misma que la de los resultados de búsqueda)
    image: Option<String>,
    /// Imagen ancha usada como fondo de la vista de detalle
    hero: Option<String>,
}

struct AniGpuApp {
    rt: Arc<Runtime>,
    items: Option<Vec<LatestItem>>,
    is_loading: bool,
    error_msg: Option<String>,
    rx: Receiver<Result<Vec<LatestItem>, String>>,
    tx: Sender<Result<Vec<LatestItem>, String>>,
    focused_index: usize,
    selected_tab: usize,

    // Búsqueda de animes
    search_query: String,
    search_results: Vec<CardItem>,
    search_loading: bool,
    search_error: Option<String>,
    /// Última consulta ya disparada (para no repetir peticiones)
    search_sent: String,
    /// Momento del último cambio de texto (debounce)
    search_changed_at: std::time::Instant,
    /// Poner el foco en el input la próxima vez que se pinte la vista
    search_focus: bool,
    rx_search: Receiver<(String, Result<Vec<CardItem>, String>)>,
    tx_search: Sender<(String, Result<Vec<CardItem>, String>)>,

    tmdb_cache: std::collections::HashMap<String, String>,
    rx_tmdb: Receiver<(String, String)>,
    tx_tmdb: Sender<(String, String)>,
    tmdb_requested: std::collections::HashSet<String>,

    // Progreso de visualización por anime (watch_progress.json)
    watch_progress: std::collections::HashMap<String, WatchProgress>,

    // Navigation
    screen: Screen,

    // Detail view state
    detail_data: Option<AnimeDetails>,
    detail_loading: bool,
    detail_error: Option<String>,
    detail_tab: DetailTab,
    rx_detail: Receiver<Result<AnimeDetails, String>>,
    tx_detail: Sender<Result<AnimeDetails, String>>,
    detail_backdrop_url: Option<String>,
    detail_backdrop_requested: bool,
}

impl AniGpuApp {
    fn cache_file_path() -> Option<std::path::PathBuf> {
        directories::ProjectDirs::from("", "", "AniGPU")
            .map(|dirs| dirs.cache_dir().join("tmdb_backdrops.json"))
    }

    fn load_tmdb_cache() -> std::collections::HashMap<String, String> {
        if let Some(path) = Self::cache_file_path() {
            if path.exists() {
                if let Ok(data) = std::fs::read_to_string(&path) {
                    if let Ok(map) =
                        serde_json::from_str::<std::collections::HashMap<String, String>>(&data)
                    {
                        return map;
                    }
                }
            }
        }
        std::collections::HashMap::new()
    }

    fn save_tmdb_cache(&self) {
        if let Some(path) = Self::cache_file_path() {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string(&self.tmdb_cache) {
                let _ = std::fs::write(&path, json);
            }
        }
    }

    // ── Progreso de visualización ────────────────────────────────────────────

    fn watch_progress_path() -> Option<std::path::PathBuf> {
        directories::ProjectDirs::from("", "", "AniGPU")
            .map(|dirs| dirs.cache_dir().join("watch_progress.json"))
    }

    fn load_watch_progress() -> std::collections::HashMap<String, WatchProgress> {
        if let Some(path) = Self::watch_progress_path() {
            if path.exists() {
                if let Ok(data) = std::fs::read_to_string(&path) {
                    if let Ok(map) = serde_json::from_str::<
                        std::collections::HashMap<String, WatchProgress>,
                    >(&data)
                    {
                        return map;
                    }
                }
            }
        }
        std::collections::HashMap::new()
    }

    fn save_watch_progress(&self) {
        if let Some(path) = Self::watch_progress_path() {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string(&self.watch_progress) {
                let _ = std::fs::write(&path, json);
            }
        }
    }

    /// Clave estable del anime abierto en la vista de detalle.
    fn anime_key(&self, details: &AnimeDetails) -> String {
        match &self.screen {
            Screen::Detail { anime_url, .. } => anime_url.clone(),
            _ => details.title.clone(),
        }
    }

    /// Número de un episodio (o su posición si la fuente no trae número).
    fn episode_number(ep: &EpisodeItem, idx: usize) -> i64 {
        ep.episode.as_i64().unwrap_or((idx + 1) as i64)
    }

    /// El usuario arrancó este episodio: queda "en curso" (no lo marca completado).
    fn mark_episode_started(&mut self, anime: &str, ep: i64) {
        let entry = self.watch_progress.entry(anime.to_owned()).or_default();
        if entry.current.as_ref().map(|c| c.ep) == Some(ep) {
            return;
        }
        entry.current = Some(CurrentEpisode { ep, position: 0.0 });
        self.save_watch_progress();
    }

    /// API preparada para el player (fase mpv): marca el capítulo como visto por completo.
    #[allow(dead_code)]
    fn mark_episode_completed(&mut self, anime: &str, ep: i64) {
        let entry = self.watch_progress.entry(anime.to_owned()).or_default();
        if !entry.completed.contains(&ep) {
            entry.completed.push(ep);
            entry.completed.sort_unstable();
        }
        if entry.current.as_ref().map(|c| c.ep) == Some(ep) {
            entry.current = None;
        }
        self.save_watch_progress();
    }

    /// Texto del botón Reproducir + índice del episodio al que lleva:
    /// - nada visto → "Reproducir - Capítulo 1"
    /// - capítulo a medias → "Continuar viendo - Capítulo N"
    /// - N completados → "Reproducir - Capítulo N+1"
    fn play_button_state(
        progress: Option<&WatchProgress>,
        details: &AnimeDetails,
    ) -> (String, Option<usize>) {
        let episodes = &details.episodes;
        if episodes.is_empty() {
            return ("▶  Reproducir".to_string(), None);
        }

        let number_at = |idx: usize| Self::episode_number(&episodes[idx], idx);
        let label_at = |idx: usize, prefix: &str| format!("{}{}", prefix, episodes[idx].episode);
        let play = "▶  Reproducir - Capítulo ";

        match progress {
            None => (label_at(0, play), Some(0)),
            Some(progress) => {
                // Capítulo empezado y sin terminar
                if let Some(current) = &progress.current {
                    if let Some(idx) = (0..episodes.len()).find(|&i| number_at(i) == current.ep) {
                        return (label_at(idx, "▶  Continuar viendo - Capítulo "), Some(idx));
                    }
                }

                // Último capítulo completado + 1 (si ya no existe, el último)
                if let Some(&last) = progress.completed.iter().max() {
                    let next = last + 1;
                    let idx = (0..episodes.len())
                        .find(|&i| number_at(i) == next)
                        .unwrap_or(episodes.len() - 1);
                    return (label_at(idx, play), Some(idx));
                }

                (label_at(0, play), Some(0))
            }
        }
    }

    fn new(rt: Arc<Runtime>) -> Self {
        let (tx, rx) = channel();
        let (tx_tmdb, rx_tmdb) = channel();
        let (tx_detail, rx_detail) = channel();
        let (tx_search, rx_search) = channel();
        let tmdb_cache = Self::load_tmdb_cache();
        let tmdb_requested: std::collections::HashSet<String> =
            tmdb_cache.keys().cloned().collect();
        Self {
            rt,
            items: None,
            is_loading: true,
            error_msg: None,
            rx,
            tx,
            focused_index: 0,
            selected_tab: 0,

            search_query: String::new(),
            search_results: Vec::new(),
            search_loading: false,
            search_error: None,
            search_sent: String::new(),
            search_changed_at: std::time::Instant::now(),
            search_focus: false,
            rx_search,
            tx_search,

            tmdb_cache,
            rx_tmdb,
            tx_tmdb,
            tmdb_requested,

            watch_progress: Self::load_watch_progress(),

            screen: Screen::Home,

            detail_data: None,
            detail_loading: false,
            detail_error: None,
            detail_tab: DetailTab::Info,
            rx_detail,
            tx_detail,
            detail_backdrop_url: None,
            detail_backdrop_requested: false,
        }
    }

    fn load_latest(&mut self, ctx: egui::Context) {
        self.is_loading = true;
        self.error_msg = None;
        let tx = self.tx.clone();

        self.rt.spawn(async move {
            let source = get_source(Some("animeav1"));
            match source.get_latest().await {
                Ok(items) => {
                    let _ = tx.send(Ok(items));
                }
                Err(e) => {
                    let _ = tx.send(Err(e.to_string()));
                }
            }
            ctx.request_repaint();
        });
    }

    fn navigate_to_detail(&mut self, item: &LatestItem, ctx: egui::Context) {
        let anime_url = item.anime_url.clone().unwrap_or_else(|| item.url.clone());

        self.screen = Screen::Detail {
            anime_url: anime_url.clone(),
            origin_item: item.clone(),
        };
        self.detail_data = None;
        self.detail_loading = true;
        self.detail_error = None;
        self.detail_tab = DetailTab::Info;
        self.detail_backdrop_url = None;
        self.detail_backdrop_requested = false;

        let tx = self.tx_detail.clone();
        self.rt.spawn(async move {
            let source = get_source(Some("animeav1"));
            match source.get_details(&anime_url).await {
                Ok(details) => {
                    let _ = tx.send(Ok(details));
                }
                Err(e) => {
                    let _ = tx.send(Err(e.to_string()));
                }
            }
            ctx.request_repaint();
        });
    }

    async fn fetch_tmdb_backdrop(title: &str) -> Option<String> {
        let api_key = "647aef6fffac587fb62b2057cf9347aa";

        // Lista de candidatos de búsqueda en orden de especificidad
        let mut candidates = Vec::new();

        // 1. Limpieza de sufijos comunes de doblaje / formato
        let base_clean = title
            .replace(" (TV)", "")
            .replace(" (TV 2024)", "")
            .replace(" (TV 2025)", "")
            .replace(" (Audio Latino)", "")
            .replace(" (Latino)", "")
            .replace(" (Castellano)", "")
            .replace(" (Sub Español)", "")
            .replace(" (Doblaje)", "")
            .trim()
            .to_string();

        candidates.push(base_clean.clone());

        // 2. Si contiene "Season", "Temporada", "Part", etc., extraer el título base
        let season_patterns = [
        regex::Regex::new(r"(?i)\s+(?:(?:\d+(?:st|nd|rd|th)?|final|second|third|fourth|segunda|tercera|cuarta)\s+)?(?:season|temporada|part|parte|cour).*").unwrap(),
        regex::Regex::new(r"(?i)\s+(?:season|temporada)\s+\d+.*").unwrap(),
        regex::Regex::new(r"(?i)\s+s\d+.*").unwrap(),
    ];

        let mut stripped_season = base_clean.clone();
        for pat in &season_patterns {
            if pat.is_match(&stripped_season) {
                stripped_season = pat.replace(&stripped_season, "").trim().to_string();
            }
        }
        if !stripped_season.is_empty() && stripped_season != base_clean {
            candidates.push(stripped_season.clone());
        }

        // 3. Extraer prefijos antes de ":" o "：" (dos puntos) y guiones (" - ", " – ", " — ")
        let current_list = candidates.clone();
        for s in &current_list {
            if let Some(pos) = s.find(':').or_else(|| s.find('：')) {
                let prefix = s[..pos].trim().to_string();
                if prefix.len() >= 3 && !candidates.contains(&prefix) {
                    candidates.push(prefix.clone());

                    let mut p_clean = prefix.clone();
                    for pat in &season_patterns {
                        if pat.is_match(&p_clean) {
                            p_clean = pat.replace(&p_clean, "").trim().to_string();
                        }
                    }
                    if !p_clean.is_empty() && !candidates.contains(&p_clean) {
                        candidates.push(p_clean);
                    }
                }
            }

            for sep in [" - ", " – ", " — "] {
                if let Some(pos) = s.find(sep) {
                    let prefix = s[..pos].trim().to_string();
                    if prefix.len() >= 3 && !candidates.contains(&prefix) {
                        candidates.push(prefix);
                    }
                }
            }
        }

        // Realizar búsquedas en TMDB por cada candidato, priorizando siempre Series (TV) sobre Películas
        for query in &candidates {
            if query.is_empty() {
                continue;
            }

            let endpoints = ["search/tv", "search/multi"];
            let languages = [Some("es-MX"), None];

            for endpoint in &endpoints {
                for lang in &languages {
                    let mut url = format!(
                        "https://api.themoviedb.org/3/{}?api_key={}&query={}&include_adult=false",
                        endpoint,
                        api_key,
                        urlencoding::encode(query)
                    );
                    if let Some(l) = lang {
                        url.push_str(&format!("&language={}", l));
                    }

                    if let Ok(resp) = reqwest::get(&url).await {
                        if let Ok(json) = resp.json::<serde_json::Value>().await {
                            if let Some(results) = json["results"].as_array() {
                                // 1ra pasada: Prioridad estricta a series de TV / Anime
                                for res in results {
                                    let media_type = res["media_type"].as_str().unwrap_or("tv");
                                    if media_type == "tv" {
                                        if let Some(path) = res["backdrop_path"].as_str() {
                                            return Some(format!(
                                                "https://image.tmdb.org/t/p/original{}",
                                                path
                                            ));
                                        }
                                    }
                                }

                                // 2da pasada: Si no hubo serie, aceptar película si tiene fondo
                                for res in results {
                                    if let Some(path) = res["backdrop_path"].as_str() {
                                        return Some(format!(
                                            "https://image.tmdb.org/t/p/original{}",
                                            path
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // 4. Fallback con base de datos de anime (Kitsu API) para resolver nombres alternativos/en inglés (ej: "Nijusseiki Denki Mokuroku" -> "Sparks of Tomorrow")
        for query in &candidates {
            if query.is_empty() {
                continue;
            }
            let kitsu_url = format!(
                "https://kitsu.io/api/edge/anime?filter[text]={}&page[limit]=1",
                urlencoding::encode(query)
            );

            if let Ok(resp) = reqwest::get(&kitsu_url).await {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    if let Some(data) = json["data"].as_array() {
                        if let Some(first) = data.first() {
                            let attrs = &first["attributes"];

                            // Obtener títulos alternativos en inglés
                            let mut alt_names = Vec::new();
                            if let Some(en) = attrs["titles"]["en"].as_str() {
                                alt_names.push(en);
                            }
                            if let Some(en_us) = attrs["titles"]["en_us"].as_str() {
                                alt_names.push(en_us);
                            }
                            if let Some(canonical) = attrs["canonicalTitle"].as_str() {
                                alt_names.push(canonical);
                            }

                            for alt in alt_names {
                                if !candidates.contains(&alt.to_string()) {
                                    let tmdb_url = format!(
                                        "https://api.themoviedb.org/3/search/tv?api_key={}&query={}&include_adult=false",
                                        api_key,
                                        urlencoding::encode(alt)
                                    );
                                    if let Ok(t_resp) = reqwest::get(&tmdb_url).await {
                                        if let Ok(t_json) = t_resp.json::<serde_json::Value>().await
                                        {
                                            if let Some(t_results) = t_json["results"].as_array() {
                                                for res in t_results {
                                                    if let Some(path) =
                                                        res["backdrop_path"].as_str()
                                                    {
                                                        return Some(format!(
                                                            "https://image.tmdb.org/t/p/original{}",
                                                            path
                                                        ));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            // Si TMDB aún no tiene backdrop, usar el coverImage oficial de Kitsu (alta resolución)
                            if let Some(cover_url) = attrs["coverImage"]["original"]
                                .as_str()
                                .or_else(|| attrs["coverImage"]["large"].as_str())
                            {
                                return Some(cover_url.to_string());
                            }
                        }
                    }
                }
            }
        }

        None
    }

    fn render_sidebar(&mut self, ui: &mut egui::Ui) {
        let sidebar_width = ui.available_width();

        // --- 1. Logo superior ---
        ui.add_space(8.0);
        let (logo_rect, logo_resp) =
            ui.allocate_exact_size(egui::vec2(sidebar_width, 36.0), egui::Sense::click());
        let center = logo_rect.center();

        // Isotipo geométrico minimalista (cubo isométrico facetado)
        let p = ui.painter();
        let size = 9.5;
        let col_top = if logo_resp.hovered() {
            egui::Color32::WHITE
        } else {
            egui::Color32::from_rgb(225, 225, 235)
        };
        let col_left = egui::Color32::from_rgb(140, 140, 160);
        let col_right = egui::Color32::from_rgb(85, 85, 105);

        p.add(egui::Shape::convex_polygon(
            vec![
                center + egui::vec2(0.0, -size),
                center + egui::vec2(size * 0.866, -size * 0.5),
                center,
                center + egui::vec2(-size * 0.866, -size * 0.5),
            ],
            col_top,
            egui::Stroke::NONE,
        ));
        p.add(egui::Shape::convex_polygon(
            vec![
                center + egui::vec2(-size * 0.866, -size * 0.5),
                center,
                center + egui::vec2(0.0, size),
                center + egui::vec2(-size * 0.866, size * 0.5),
            ],
            col_left,
            egui::Stroke::NONE,
        ));
        p.add(egui::Shape::convex_polygon(
            vec![
                center,
                center + egui::vec2(size * 0.866, -size * 0.5),
                center + egui::vec2(size * 0.866, size * 0.5),
                center + egui::vec2(0.0, size),
            ],
            col_right,
            egui::Stroke::NONE,
        ));

        // --- 2. Menú de Navegación Central ---
        let total_avail = ui.available_height();
        let mid_spacing = (total_avail * 0.32).clamp(35.0, 180.0);
        ui.add_space(mid_spacing);

        let btn_size = egui::vec2(38.0, 38.0);

        // Tab 0: Tarjetas / Inicio (Estilo 'Boards')
        ui.vertical_centered(|ui| {
            let is_active = self.selected_tab == 0;
            let (rect, resp) = ui.allocate_exact_size(btn_size, egui::Sense::click());

            if resp.clicked() {
                self.selected_tab = 0;
                self.screen = Screen::Home;
            }

            if is_active {
                ui.painter()
                    .rect_filled(rect, 9.0, egui::Color32::from_rgb(32, 32, 38));
            } else if resp.hovered() {
                ui.painter()
                    .rect_filled(rect, 9.0, egui::Color32::from_rgb(22, 22, 28));
            }

            let icon_center = rect.center();
            let card_col = if is_active {
                egui::Color32::WHITE
            } else if resp.hovered() {
                egui::Color32::from_rgb(200, 200, 210)
            } else {
                egui::Color32::from_rgb(110, 110, 125)
            };

            // Carta trasera
            let back_rect = egui::Rect::from_center_size(
                icon_center + egui::vec2(-2.5, -2.0),
                egui::vec2(12.0, 15.0),
            );
            ui.painter().rect_stroke(
                back_rect,
                3.0,
                egui::Stroke::new(1.4, card_col.gamma_multiply(0.55)),
                egui::StrokeKind::Outside,
            );

            // Carta frontal
            let front_rect = egui::Rect::from_center_size(
                icon_center + egui::vec2(2.5, 2.0),
                egui::vec2(12.0, 15.0),
            );
            ui.painter().rect_filled(
                front_rect,
                3.0,
                if is_active {
                    egui::Color32::from_rgb(32, 32, 38)
                } else {
                    egui::Color32::from_rgb(10, 10, 13)
                },
            );
            ui.painter().rect_stroke(
                front_rect,
                3.0,
                egui::Stroke::new(1.4, card_col),
                egui::StrokeKind::Outside,
            );
        });

        ui.add_space(12.0);

        // Tab 1: Cuadrícula / Catálogo (Grid 2x2)
        ui.vertical_centered(|ui| {
            let is_active = self.selected_tab == 1;
            let (rect, resp) = ui.allocate_exact_size(btn_size, egui::Sense::click());

            if resp.clicked() {
                self.selected_tab = 1;
            }

            if is_active {
                ui.painter()
                    .rect_filled(rect, 9.0, egui::Color32::from_rgb(32, 32, 38));
            } else if resp.hovered() {
                ui.painter()
                    .rect_filled(rect, 9.0, egui::Color32::from_rgb(22, 22, 28));
            }

            let icon_center = rect.center();
            let grid_col = if is_active {
                egui::Color32::WHITE
            } else if resp.hovered() {
                egui::Color32::from_rgb(200, 200, 210)
            } else {
                egui::Color32::from_rgb(110, 110, 125)
            };

            let offsets = [
                egui::vec2(-4.2, -4.2),
                egui::vec2(4.2, -4.2),
                egui::vec2(-4.2, 4.2),
                egui::vec2(4.2, 4.2),
            ];
            for off in offsets {
                let sq_rect = egui::Rect::from_center_size(icon_center + off, egui::vec2(5.5, 5.5));
                ui.painter().rect_filled(sq_rect, 1.2, grid_col);
            }
        });

        ui.add_space(12.0);

        // Tab 2: Buscar (lupa)
        ui.vertical_centered(|ui| {
            let is_active = self.selected_tab == 2;
            let (rect, resp) = ui.allocate_exact_size(btn_size, egui::Sense::click());

            if resp.clicked() {
                self.selected_tab = 2;
                self.search_focus = true;
                self.screen = Screen::Search;
            }

            if is_active {
                ui.painter()
                    .rect_filled(rect, 9.0, egui::Color32::from_rgb(32, 32, 38));
            } else if resp.hovered() {
                ui.painter()
                    .rect_filled(rect, 9.0, egui::Color32::from_rgb(22, 22, 28));
            }

            let search_col = if is_active {
                egui::Color32::WHITE
            } else if resp.hovered() {
                egui::Color32::from_rgb(200, 200, 210)
            } else {
                egui::Color32::from_rgb(110, 110, 125)
            };

            // Lupa: círculo + mango
            let glass_center = rect.center() + egui::vec2(-1.8, -1.8);
            ui.painter().circle_stroke(
                glass_center,
                6.0,
                egui::Stroke::new(1.9, search_col),
            );
            ui.painter().line_segment(
                [
                    glass_center + egui::vec2(4.3, 4.3),
                    rect.center() + egui::vec2(7.5, 7.5),
                ],
                egui::Stroke::new(1.9, search_col),
            );
        });

        // --- 3. Sección Inferior (Ayuda + Perfil) ---
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
            ui.add_space(10.0);

            // Perfil / Avatar
            let (p_rect, p_resp) =
                ui.allocate_exact_size(egui::vec2(30.0, 30.0), egui::Sense::click());
            let p_center = p_rect.center();
            let p_col = if p_resp.hovered() {
                egui::Color32::WHITE
            } else {
                egui::Color32::from_rgb(120, 120, 135)
            };

            ui.painter()
                .circle_filled(p_center, 14.0, egui::Color32::from_rgb(22, 22, 28));
            ui.painter().circle_stroke(
                p_center,
                14.0,
                egui::Stroke::new(
                    1.0,
                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, 25),
                ),
            );

            ui.painter()
                .circle_filled(p_center + egui::vec2(0.0, -3.0), 4.0, p_col);
            ui.painter()
                .circle_filled(p_center + egui::vec2(0.0, 7.5), 6.5, p_col);

            ui.add_space(12.0);

            // Ayuda / Info (?)
            let (h_rect, h_resp) =
                ui.allocate_exact_size(egui::vec2(26.0, 26.0), egui::Sense::click());
            let h_center = h_rect.center();
            let h_col = if h_resp.hovered() {
                egui::Color32::WHITE
            } else {
                egui::Color32::from_rgb(110, 110, 125)
            };

            ui.painter()
                .circle_stroke(h_center, 9.5, egui::Stroke::new(1.2, h_col));
            ui.painter().text(
                h_center + egui::vec2(0.0, -0.5),
                egui::Align2::CENTER_CENTER,
                "?",
                egui::FontId::proportional(11.0),
                h_col,
            );
        });
    }

    // ═══════════════════════════════════════════════════════════════════════════
    //  SEARCH VIEW (input + sugerencias + grid de resultados)
    // ═══════════════════════════════════════════════════════════════════════════

    fn render_search_view(&mut self, ui: &mut egui::Ui) {
        // Escape → volver al inicio
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.screen = Screen::Home;
            self.selected_tab = 0;
            return;
        }

        // Resultado que llega del canal (solo si corresponde a la última consulta)
        if let Ok((query, result)) = self.rx_search.try_recv() {
            if query == self.search_sent {
                self.search_loading = false;
                match result {
                    Ok(items) => {
                        self.search_error = None;
                        self.search_results = items;
                    }
                    Err(e) => self.search_error = Some(e),
                }
            }
        }

        let avail_width = ui.available_width();
        let avail_height = ui.available_height();
        let margin = (avail_width * 0.03).clamp(24.0, 48.0);
        let left_w = (avail_width * 0.26).clamp(280.0, 380.0);
        let gap = 28.0;

        let (root, _) =
            ui.allocate_exact_size(egui::vec2(avail_width, avail_height), egui::Sense::hover());

        // ── Columna izquierda: input + sugerencias ───────────────────
        let input_rect = egui::Rect::from_min_size(
            egui::pos2(root.min.x + margin, root.min.y + margin),
            egui::vec2(left_w - margin, 46.0),
        );
        let suggest_rect = egui::Rect::from_min_max(
            egui::pos2(input_rect.min.x, input_rect.max.y + 18.0),
            egui::pos2(input_rect.max.x, root.max.y - margin),
        );

        // ── Derecha: grid de pósters ─────────────────────────────────
        let grid_rect = egui::Rect::from_min_max(
            egui::pos2(root.min.x + left_w + gap, root.min.y + margin),
            egui::pos2(root.max.x - margin, root.max.y - margin),
        );

        // Input de búsqueda
        let mut input_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(input_rect)
                .layout(egui::Layout::top_down(egui::Align::LEFT)),
        );
        let output = egui::TextEdit::singleline(&mut self.search_query)
            .hint_text("Buscar animes…")
            .font(egui::FontId::proportional(17.0))
            .desired_width(input_rect.width())
            .show(&mut input_ui);

        if output.response.changed() {
            self.search_changed_at = std::time::Instant::now();
        }
        if self.search_focus {
            output.response.request_focus();
            self.search_focus = false;
        }

        // ── Búsqueda con debounce de 450 ms ──────────────────────────
        let query = self.search_query.trim().to_string();
        if query.chars().count() < 2 {
            if self.search_loading || !self.search_results.is_empty() {
                self.search_results.clear();
                self.search_error = None;
                self.search_loading = false;
                self.search_sent.clear();
            }
        } else if self.search_sent != query
            && self.search_changed_at.elapsed() >= std::time::Duration::from_millis(450)
        {
            self.search_sent = query.clone();
            self.search_loading = true;
            self.search_error = None;

            let sent = query.clone();
            let tx = self.tx_search.clone();
            let ctx = ui.ctx().clone();
            self.rt.spawn(async move {
                let result = get_source(Some("animeav1"))
                    .search(&sent)
                    .await
                    .map_err(|e| e.to_string());
                let _ = tx.send((sent, result));
                ctx.request_repaint();
            });
        }

        // ── Filas compartidas por sugerencias y grid ─────────────────
        let lowered = query.to_lowercase();
        let rows: Vec<SearchRow> = if lowered.is_empty() {
            // Sin consulta todavía: mostramos lo último como sugerencias
            self.items
                .clone()
                .unwrap_or_default()
                .iter()
                .map(|item| SearchRow {
                    title: item.title.clone(),
                    url: item.url.clone(),
                    anime_url: item.anime_url.clone(),
                    image: item
                        .image
                        .as_deref()
                        .and_then(cover_from_thumb)
                        .or_else(|| item.cover.clone())
                        .or_else(|| item.image.clone()),
                    hero: item.image.clone(),
                })
                .collect()
        } else {
            self.search_results
                .iter()
                .filter(|card| card.title.to_lowercase().contains(&lowered))
                .map(|card| SearchRow {
                    title: card.title.clone(),
                    url: card.url.clone(),
                    anime_url: card.anime_url.clone(),
                    image: card.image.clone(),
                    hero: card.image.clone(),
                })
                .collect()
        };

        let mut nav_target: Option<SearchRow> = None;

        // ── Sugerencias (columna izquierda) ──────────────────────────
        let mut sugg_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(suggest_rect)
                .layout(egui::Layout::top_down(egui::Align::LEFT)),
        );

        let row_h = 38.0;
        egui::ScrollArea::vertical()
            .id_salt("search_suggestions")
            .auto_shrink([false, false])
            .show(&mut sugg_ui, |ui| {
                if rows.is_empty() {
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new(if lowered.is_empty() {
                            "Escribe para buscar…"
                        } else {
                            "Sin sugerencias"
                        })
                        .color(egui::Color32::from_rgb(140, 140, 155))
                        .size(14.0),
                    );
                }

                for row in &rows {
                    let (rect, resp) = ui.allocate_exact_size(
                        egui::vec2(suggest_rect.width() - 12.0, row_h),
                        egui::Sense::click(),
                    );

                    if resp.hovered() {
                        ui.painter().rect_filled(
                            rect,
                            6.0,
                            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 12),
                        );
                    }
                    if resp.clicked() {
                        nav_target = Some(row.clone());
                    }

                    let text_rect = egui::Rect::from_min_max(
                        egui::pos2(rect.min.x + 12.0, rect.min.y),
                        egui::pos2(rect.max.x - 8.0, rect.max.y),
                    );
                    let mut text_ui = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(text_rect)
                            .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    );
                    text_ui.add(
                        egui::Label::new(
                            egui::RichText::new(&row.title)
                                .color(egui::Color32::from_rgb(210, 210, 225))
                                .size(15.0),
                        )
                        .truncate(),
                    );
                }

                ui.add_space(20.0);
            });

        // ── Grid de resultados (derecha) ─────────────────────────────
        let card_h = (avail_height * 0.32).clamp(180.0, 300.0);
        let card_w = card_h * 0.70;
        let spacing = 16.0;
        let grid_w = grid_rect.width() - 14.0; // hueco para la barra de scroll
        let per_row = ((grid_w + spacing) / (card_w + spacing))
            .floor()
            .max(1.0) as usize;

        if rows.is_empty() {
            let mut empty_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(grid_rect)
                    .layout(egui::Layout::top_down(egui::Align::LEFT)),
            );
            empty_ui.centered_and_justified(|ui| {
                if self.search_loading {
                    ui.add(egui::Spinner::new());
                } else {
                    ui.label(
                        egui::RichText::new(if self.search_error.is_some() {
                            "No se pudo completar la búsqueda"
                        } else if lowered.is_empty() {
                            "Escribe algo para buscar…"
                        } else {
                            "Sin resultados"
                        })
                        .color(egui::Color32::from_rgb(150, 150, 165))
                        .size(16.0),
                    );
                }
            });
        } else {
            let mut grid_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(grid_rect)
                    .layout(egui::Layout::top_down(egui::Align::LEFT)),
            );

            egui::ScrollArea::vertical()
                .id_salt("search_grid")
                .auto_shrink([false, false])
                .show(&mut grid_ui, |ui| {
                    for chunk in rows.chunks(per_row) {
                        ui.horizontal(|ui| {
                            for row in chunk {
                                let (card_rect, resp) = ui.allocate_exact_size(
                                    egui::vec2(card_w, card_h),
                                    egui::Sense::click(),
                                );

                                if resp.clicked() {
                                    nav_target = Some(row.clone());
                                }

                                if let Some(img) = &row.image {
                                    egui::Image::new(img)
                                        .fit_to_exact_size(card_rect.size())
                                        .corner_radius(8.0)
                                        .paint_at(ui, card_rect);
                                } else {
                                    ui.painter().rect_filled(
                                        card_rect,
                                        8.0,
                                        egui::Color32::from_rgb(26, 26, 33),
                                    );
                                }

                                let stroke = if resp.hovered() {
                                    egui::Stroke::new(3.0, egui::Color32::WHITE)
                                } else {
                                    egui::Stroke::new(
                                        1.5,
                                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 45),
                                    )
                                };
                                ui.painter().rect_stroke(
                                    card_rect,
                                    8.0,
                                    stroke,
                                    egui::StrokeKind::Outside,
                                );

                                ui.add_space(spacing);
                            }
                        });
                        ui.add_space(spacing);
                    }

                    ui.add_space(20.0);
                });
        }

        // ── Abrir la ficha del anime elegido ─────────────────────────
        if let Some(row) = nav_target {
            let item = LatestItem {
                title: row.title,
                episode: None,
                image: row.hero,
                cover: row.image,
                anime_url: row.anime_url,
                url: row.url,
            };
            let ctx = ui.ctx().clone();
            self.navigate_to_detail(&item, ctx);
        }
    }

    // ═══════════════════════════════════════════════════════════════════════════
    //  DETAIL VIEW (Netflix-style)
    // ═══════════════════════════════════════════════════════════════════════════

    fn render_detail_view(&mut self, ui: &mut egui::Ui) {
        // Process incoming detail data
        if let Ok(result) = self.rx_detail.try_recv() {
            self.detail_loading = false;
            match result {
                Ok(details) => self.detail_data = Some(details),
                Err(e) => self.detail_error = Some(e),
            }
        }

        let avail_width = ui.available_width();
        let avail_height = ui.available_height();

        // Back button / Escape key
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.detail_tab == DetailTab::Info {
                self.screen = Screen::Home;
            } else {
                self.detail_tab = DetailTab::Info;
            }
            return;
        }

        // Retrieve origin item info for fallbacks
        let (_origin_title, _origin_ep, origin_image) = match &self.screen {
            Screen::Detail { origin_item, .. } => (
                origin_item.title.clone(),
                origin_item.episode.clone(),
                origin_item.image.clone(),
            ),
            _ => (String::new(), None, None),
        };

        // Loading state
        if self.detail_loading {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return;
        }

        // Error state
        if let Some(err) = &self.detail_error {
            let err = err.clone();
            ui.vertical_centered(|ui| {
                ui.add_space(avail_height * 0.3);
                ui.label(
                    egui::RichText::new("⚠  No se pudo cargar")
                        .color(egui::Color32::from_rgb(255, 120, 120))
                        .size(22.0)
                        .strong(),
                );
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(&err)
                        .color(egui::Color32::from_rgb(160, 160, 175))
                        .size(13.0),
                );
                ui.add_space(20.0);
                ui.horizontal(|ui| {
                    ui.add_space(avail_width * 0.5 - 140.0);
                    let back_btn = egui::Button::new(
                        egui::RichText::new("←  Volver")
                            .color(egui::Color32::WHITE)
                            .size(14.0),
                    )
                    .fill(egui::Color32::from_rgb(40, 40, 50))
                    .corner_radius(6.0);
                    if ui.add_sized([120.0, 36.0], back_btn).clicked() {
                        self.screen = Screen::Home;
                    }

                    ui.add_space(8.0);

                    let retry_btn = egui::Button::new(
                        egui::RichText::new("↻  Reintentar")
                            .color(egui::Color32::WHITE)
                            .size(14.0),
                    )
                    .fill(egui::Color32::from_rgb(40, 40, 50))
                    .corner_radius(6.0);
                    if ui.add_sized([130.0, 36.0], retry_btn).clicked() {
                        if let Screen::Detail {
                            ref anime_url,
                            ref origin_item,
                        } = self.screen.clone()
                        {
                            let item = origin_item.clone();
                            let ctx = ui.ctx().clone();
                            self.navigate_to_detail(&item, ctx);
                        }
                    }
                });
            });
            return;
        }

        let details = match &self.detail_data {
            Some(d) => d.clone(),
            None => return,
        };

        // Fetch backdrop via TMDB if needed
        if !self.detail_backdrop_requested {
            self.detail_backdrop_requested = true;
            let title = details.title.clone();
            if let Some(cached) = self.tmdb_cache.get(&title) {
                self.detail_backdrop_url = if cached.is_empty() {
                    None
                } else {
                    Some(cached.clone())
                };
            } else {
                let tx_tmdb = self.tx_tmdb.clone();
                let ctx = ui.ctx().clone();
                self.rt.spawn(async move {
                    let found = Self::fetch_tmdb_backdrop(&title).await;
                    if let Some(u) = found {
                        let _ = tx_tmdb.send((title, u));
                        ctx.request_repaint();
                    } else {
                        let _ = tx_tmdb.send((title, "".to_string()));
                    }
                });
            }
        }

        // Check tmdb cache for newly arrived backdrop
        if self.detail_backdrop_url.is_none() {
            if let Some(cached) = self.tmdb_cache.get(&details.title) {
                if !cached.is_empty() {
                    self.detail_backdrop_url = Some(cached.clone());
                }
            }
        }

        // Determine the best backdrop URL
        let backdrop_url = self
            .detail_backdrop_url
            .clone()
            .or_else(|| details.backdrop.clone())
            .or_else(|| details.cover.clone())
            .or(origin_image);

        let bg_color = ui.visuals().panel_fill;

        // ── Render based on current tab ─────────────────────────────────
        match self.detail_tab {
            DetailTab::Info => {
                self.render_detail_info(
                    ui,
                    &details,
                    &backdrop_url,
                    bg_color,
                    avail_width,
                    avail_height,
                );
            }
            DetailTab::Episodes => {
                self.render_detail_episodes(
                    ui,
                    &details,
                    &backdrop_url,
                    bg_color,
                    avail_width,
                    avail_height,
                );
            }
            DetailTab::Related => {
                self.render_detail_related(
                    ui,
                    &details,
                    &backdrop_url,
                    bg_color,
                    avail_width,
                    avail_height,
                );
            }
        }
    }

    /// INFO tab: big backdrop with title, synopsis, genres, play button, etc.
    fn render_detail_info(
        &mut self,
        ui: &mut egui::Ui,
        details: &AnimeDetails,
        backdrop_url: &Option<String>,
        bg_color: egui::Color32,
        avail_width: f32,
        avail_height: f32,
    ) {
        let hero_height = avail_height;
        let margin_x = (avail_width * 0.04).clamp(28.0, 60.0);

        // ── Full-screen backdrop behind everything ────────────────
        let full_bg_rect =
            egui::Rect::from_min_size(ui.cursor().min, egui::vec2(avail_width, avail_height));
        if let Some(url) = backdrop_url {
            let image = egui::Image::new(url).fit_to_exact_size(full_bg_rect.size());
            image.paint_at(ui, full_bg_rect);
        }

        // ── Strong left-side dark gradient (very pronounced) ──────
        let left_vignette_full = egui::Rect::from_min_max(
            full_bg_rect.min,
            egui::pos2(
                full_bg_rect.min.x + (avail_width * 0.70),
                full_bg_rect.max.y,
            ),
        );
        let bg_opaque =
            egui::Color32::from_rgba_unmultiplied(bg_color.r(), bg_color.g(), bg_color.b(), 255);
        let bg_transparent =
            egui::Color32::from_rgba_unmultiplied(bg_color.r(), bg_color.g(), bg_color.b(), 0);
        Self::draw_gradient_rect(
            ui,
            left_vignette_full,
            bg_opaque,
            bg_transparent,
            bg_opaque,
            bg_transparent,
        );

        // ── Bottom fade on the full background ────────────────────
        let bottom_fade_full = egui::Rect::from_min_max(
            egui::pos2(
                full_bg_rect.min.x,
                full_bg_rect.max.y - (avail_height * 0.40),
            ),
            full_bg_rect.max,
        );
        Self::draw_gradient_rect(
            ui,
            bottom_fade_full,
            bg_transparent,
            bg_transparent,
            bg_opaque,
            bg_opaque,
        );

        let title_size = (avail_width * 0.030).clamp(26.0, 48.0);
        let sub_size = (title_size * 0.42).clamp(13.0, 17.0);
        let btn_font = (title_size * 0.40).clamp(14.0, 18.0);
        let btn_w = (avail_width * 0.14).clamp(150.0, 220.0);
        let play_w = (avail_width * 0.30).clamp(320.0, 420.0);
        let eps_w = 310.0;
        let btn_h = (avail_height * 0.06).clamp(38.0, 48.0);
        let max_content_w = (avail_width * 0.52).clamp(380.0, 680.0);

        let mut go_back = false;
        let mut go_episodes = false;
        let mut go_related = false;

        // Botón Reproducir según el progreso de visualización del usuario
        let anime_key = self.anime_key(details);
        let (play_label, play_index) =
            Self::play_button_state(self.watch_progress.get(&anime_key), details);

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space((avail_height * 0.04).clamp(24.0, 44.0));

                ui.horizontal(|ui| {
                    ui.add_space(margin_x);
                    ui.vertical(|ui| {
                        ui.set_max_width(max_content_w);

                        // ── Back button ───────────────────────────────────────
                        let back_btn = egui::Button::new(
                            egui::RichText::new("←  Volver")
                                .color(egui::Color32::from_rgb(210, 210, 230))
                                .size(sub_size),
                        )
                        .fill(egui::Color32::TRANSPARENT);

                        if ui.add(back_btn).clicked() {
                            go_back = true;
                        }

                        ui.add_space(56.0);

                        // ── Title ─────────────────────────────────────────────
                        ui.label(
                            egui::RichText::new(&details.title)
                                .color(egui::Color32::WHITE)
                                .size(title_size)
                                .strong(),
                        );

                        ui.add_space(10.0);

                        // ── Meta badges (status, genres) ──────────────────────
                        ui.horizontal_wrapped(|ui| {
                            if let Some(status) = &details.status {
                                let badge_text = egui::RichText::new(status)
                                    .color(egui::Color32::WHITE)
                                    .size(sub_size * 0.9)
                                    .strong();
                                let badge = egui::Button::new(badge_text)
                                    .fill(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 18))
                                    .corner_radius(4.0);
                                ui.add(badge);
                                ui.add_space(6.0);
                            }

                            let ep_count = details.episodes.len();
                            if ep_count > 0 {
                                ui.label(
                                    egui::RichText::new(format!("{} episodios", ep_count))
                                        .color(egui::Color32::from_rgb(180, 180, 195))
                                        .size(sub_size * 0.9),
                                );
                                ui.add_space(8.0);
                            }

                            for genre in details.genres.iter().take(4) {
                                ui.label(
                                    egui::RichText::new(genre)
                                        .color(egui::Color32::from_rgb(150, 150, 170))
                                        .size(sub_size * 0.85),
                                );
                                ui.label(
                                    egui::RichText::new("·")
                                        .color(egui::Color32::from_rgb(80, 80, 95))
                                        .size(sub_size * 0.85),
                                );
                            }
                        });

                        ui.add_space(14.0);

                        // ── Synopsis ──────────────────────────────────────────
                        if !details.synopsis.is_empty() {
                            let max_chars = 340;
                            let synopsis_text = if details.synopsis.len() > max_chars {
                                format!("{}…", &details.synopsis[..max_chars])
                            } else {
                                details.synopsis.clone()
                            };
                            ui.label(
                                egui::RichText::new(synopsis_text)
                                    .color(egui::Color32::from_rgb(190, 190, 205))
                                    .size(sub_size),
                            );
                            ui.add_space(18.0);
                        }

                        // ── Action Buttons (columna vertical) ─────────────────
                        ui.vertical(|ui| {
                            let play_btn = egui::Button::new(
                                egui::RichText::new(&play_label)
                                    .color(egui::Color32::BLACK)
                                    .size(btn_font)
                                    .strong(),
                            )
                            .fill(egui::Color32::WHITE)
                            .corner_radius(8.0);

                            if ui.add_sized([play_w, btn_h], play_btn).clicked() {
                                if let Some(idx) = play_index {
                                    let ep = &details.episodes[idx];
                                    let number = Self::episode_number(ep, idx);
                                    self.mark_episode_started(&anime_key, number);
                                    println!("Reproducir: {}", ep.url);
                                }
                            }

                            ui.add_space(10.0);

                            let ep_btn = egui::Button::new(
                                egui::RichText::new("📋  Más episodios")
                                    .color(egui::Color32::WHITE)
                                    .size(btn_font * 0.9),
                            )
                            .fill(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 15))
                            .corner_radius(8.0);

                            if ui.add_sized([eps_w, btn_h], ep_btn).clicked() {
                                go_episodes = true;
                            }

                            ui.add_space(10.0);

                            let rel_btn = egui::Button::new(
                                egui::RichText::new("⊞  Relacionados")
                                    .color(egui::Color32::WHITE)
                                    .size(btn_font * 0.9),
                            )
                            .fill(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 15))
                            .corner_radius(8.0);

                            ui.add_enabled_ui(!details.related.is_empty(), |ui| {
                                if ui.add_sized([btn_w, btn_h], rel_btn).clicked() {
                                    go_related = true;
                                }
                            });
                        });
                    });
                });

                if go_back {
                    self.screen = Screen::Home;
                    return;
                }
                if go_episodes {
                    self.detail_tab = DetailTab::Episodes;
                }
                if go_related {
                    self.detail_tab = DetailTab::Related;
                }

                ui.add_space(40.0);
            });
    }

    /// RELATED subview: header + scrollable grid of related animes
    fn render_detail_related(
        &mut self,
        ui: &mut egui::Ui,
        details: &AnimeDetails,
        backdrop_url: &Option<String>,
        bg_color: egui::Color32,
        avail_width: f32,
        avail_height: f32,
    ) {
        let margin_x = (avail_width * 0.035).clamp(24.0, 52.0);
        let title_size = (avail_width * 0.022).clamp(20.0, 36.0);
        let sub_size = (title_size * 0.50).clamp(12.0, 16.0);

        // Top mini-backdrop header
        let header_height = (avail_height * 0.22).clamp(120.0, 200.0);
        let (header_rect, _) =
            ui.allocate_exact_size(egui::vec2(avail_width, header_height), egui::Sense::hover());

        if let Some(url) = backdrop_url {
            let image = egui::Image::new(url).fit_to_exact_size(header_rect.size());
            image.paint_at(ui, header_rect);
        }

        let overlay_color =
            egui::Color32::from_rgba_unmultiplied(bg_color.r(), bg_color.g(), bg_color.b(), 200);
        ui.painter().rect_filled(header_rect, 0.0, overlay_color);

        let header_fade = egui::Rect::from_min_max(
            egui::pos2(header_rect.min.x, header_rect.max.y - 40.0),
            header_rect.max,
        );
        let transparent =
            egui::Color32::from_rgba_unmultiplied(bg_color.r(), bg_color.g(), bg_color.b(), 0);
        Self::draw_gradient_rect(
            ui,
            header_fade,
            transparent,
            transparent,
            bg_color,
            bg_color,
        );

        // Header content: back + title
        let header_content = egui::Rect::from_min_max(
            egui::pos2(header_rect.min.x + margin_x, header_rect.min.y + 16.0),
            egui::pos2(header_rect.max.x - margin_x, header_rect.max.y - 10.0),
        );
        let mut h_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(header_content)
                .layout(egui::Layout::top_down(egui::Align::LEFT)),
        );

        let back = egui::Button::new(
            egui::RichText::new("←  Volver a información")
                .color(egui::Color32::from_rgb(200, 200, 220))
                .size(sub_size),
        )
        .fill(egui::Color32::TRANSPARENT);
        if h_ui.add(back).clicked() {
            self.detail_tab = DetailTab::Info;
            return;
        }

        h_ui.add_space(6.0);
        h_ui.label(
            egui::RichText::new(&details.title)
                .color(egui::Color32::WHITE)
                .size(title_size)
                .strong(),
        );
        h_ui.add_space(4.0);
        h_ui.label(
            egui::RichText::new(format!("{} relacionados", details.related.len()))
                .color(egui::Color32::from_rgb(160, 160, 180))
                .size(sub_size),
        );

        // ── Body: grid de tarjetas ─────────────────────────────────
        let body_height = avail_height - header_height;
        let body_rect = egui::Rect::from_min_size(
            egui::pos2(header_rect.min.x, header_rect.max.y),
            egui::vec2(avail_width, body_height),
        );
        let mut body_ui = ui.new_child(egui::UiBuilder::new().max_rect(body_rect));

        let card_h = (avail_height * 0.30).clamp(180.0, 300.0);
        let card_w = card_h * 0.70;
        let card_spacing = 18.0;
        let usable_w = avail_width - margin_x * 2.0 - 18.0;
        let per_row = ((usable_w + card_spacing) / (card_w + card_spacing))
            .floor()
            .max(1.0) as usize;

        let mut nav_target: Option<LatestItem> = None;

        egui::ScrollArea::vertical()
            .id_salt("related_grid")
            .auto_shrink([false, false])
            .show(&mut body_ui, |ui| {
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    ui.add_space(margin_x);
                    ui.label(
                        egui::RichText::new("Relacionados")
                            .color(egui::Color32::WHITE)
                            .size((title_size * 0.55).clamp(16.0, 22.0))
                            .strong(),
                    );
                });
                ui.add_space(12.0);

                for row in details.related.chunks(per_row) {
                    ui.horizontal(|ui| {
                        ui.add_space(margin_x);

                        for rel in row {
                            let (card_rect, resp) = ui.allocate_exact_size(
                                egui::vec2(card_w, card_h + 44.0),
                                egui::Sense::click(),
                            );

                            if resp.clicked() {
                                nav_target = Some(LatestItem {
                                    title: rel.title.clone(),
                                    episode: None,
                                    image: rel.image.clone(),
                                    cover: None,
                                    anime_url: Some(rel.url.clone()),
                                    url: rel.url.clone(),
                                });
                            }

                            let img_rect = egui::Rect::from_min_size(
                                card_rect.min,
                                egui::vec2(card_w, card_h),
                            );

                            if let Some(img) = &rel.image {
                                let image = egui::Image::new(img)
                                    .fit_to_exact_size(img_rect.size())
                                    .corner_radius(8.0);
                                image.paint_at(ui, img_rect);
                            } else {
                                ui.painter().rect_filled(
                                    img_rect,
                                    8.0,
                                    egui::Color32::from_rgb(28, 28, 35),
                                );
                            }

                            let stroke = if resp.hovered() {
                                egui::Stroke::new(3.0, egui::Color32::WHITE)
                            } else {
                                egui::Stroke::new(
                                    1.5,
                                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, 50),
                                )
                            };
                            ui.painter().rect_stroke(
                                img_rect,
                                8.0,
                                stroke,
                                egui::StrokeKind::Outside,
                            );

                            let text_rect = egui::Rect::from_min_size(
                                egui::pos2(card_rect.min.x, img_rect.max.y + 8.0),
                                egui::vec2(card_w, 36.0),
                            );
                            let mut t_ui = ui.new_child(egui::UiBuilder::new().max_rect(text_rect));
                            t_ui.label(
                                egui::RichText::new(&rel.title)
                                    .color(egui::Color32::from_rgb(210, 210, 225))
                                    .size(13.0)
                                    .strong(),
                            );

                            ui.add_space(card_spacing);
                        }
                    });
                    ui.add_space(card_spacing);
                }

                ui.add_space(30.0);
            });

        if let Some(item) = nav_target {
            let ctx = ui.ctx().clone();
            self.navigate_to_detail(&item, ctx);
        }
    }

    /// EPISODES tab: title header on the left, scrollable episode list on the right
    fn render_detail_episodes(
        &mut self,
        ui: &mut egui::Ui,
        details: &AnimeDetails,
        backdrop_url: &Option<String>,
        bg_color: egui::Color32,
        avail_width: f32,
        avail_height: f32,
    ) {
        let margin_x = (avail_width * 0.035).clamp(24.0, 52.0);
        let title_size = (avail_width * 0.022).clamp(20.0, 36.0);
        let sub_size = (title_size * 0.50).clamp(12.0, 16.0);

        // Top mini-backdrop header
        let header_height = (avail_height * 0.22).clamp(120.0, 200.0);

        // Allocate the header
        let (header_rect, _) =
            ui.allocate_exact_size(egui::vec2(avail_width, header_height), egui::Sense::hover());

        // Draw mini backdrop
        if let Some(url) = backdrop_url {
            let image = egui::Image::new(url).fit_to_exact_size(header_rect.size());
            image.paint_at(ui, header_rect);
        }

        // Dark overlay on the header
        let overlay_color =
            egui::Color32::from_rgba_unmultiplied(bg_color.r(), bg_color.g(), bg_color.b(), 200);
        ui.painter().rect_filled(header_rect, 0.0, overlay_color);

        // Bottom fade on header
        let header_fade = egui::Rect::from_min_max(
            egui::pos2(header_rect.min.x, header_rect.max.y - 40.0),
            header_rect.max,
        );
        let transparent =
            egui::Color32::from_rgba_unmultiplied(bg_color.r(), bg_color.g(), bg_color.b(), 0);
        Self::draw_gradient_rect(
            ui,
            header_fade,
            transparent,
            transparent,
            bg_color,
            bg_color,
        );

        // Header content: title + back
        let header_content = egui::Rect::from_min_max(
            egui::pos2(header_rect.min.x + margin_x, header_rect.min.y + 16.0),
            egui::pos2(header_rect.max.x - margin_x, header_rect.max.y - 10.0),
        );
        let mut h_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(header_content)
                .layout(egui::Layout::top_down(egui::Align::LEFT)),
        );

        // Back button
        let back = egui::Button::new(
            egui::RichText::new("←  Volver a información")
                .color(egui::Color32::from_rgb(200, 200, 220))
                .size(sub_size),
        )
        .fill(egui::Color32::TRANSPARENT);
        if h_ui.add(back).clicked() {
            self.detail_tab = DetailTab::Info;
            return;
        }

        h_ui.add_space(6.0);

        h_ui.label(
            egui::RichText::new(&details.title)
                .color(egui::Color32::WHITE)
                .size(title_size)
                .strong(),
        );

        h_ui.add_space(4.0);
        h_ui.label(
            egui::RichText::new(format!("{} episodios", details.episodes.len()))
                .color(egui::Color32::from_rgb(160, 160, 180))
                .size(sub_size),
        );

        // ── Left sidebar (seasons / tabs) + Right episode list ────────
        let body_height = avail_height - header_height;
        let left_panel_w = (avail_width * 0.22).clamp(160.0, 280.0);

        // Allocate left panel
        let left_rect = egui::Rect::from_min_size(
            egui::pos2(header_rect.min.x, header_rect.max.y),
            egui::vec2(left_panel_w, body_height),
        );
        let right_rect = egui::Rect::from_min_max(
            egui::pos2(left_rect.max.x, header_rect.max.y),
            egui::pos2(header_rect.max.x, header_rect.max.y + body_height),
        );

        // Left panel background
        ui.painter()
            .rect_filled(left_rect, 0.0, egui::Color32::from_rgb(10, 10, 14));

        // Left panel content
        let left_content = egui::Rect::from_min_max(
            egui::pos2(left_rect.min.x + margin_x * 0.5, left_rect.min.y + 20.0),
            egui::pos2(left_rect.max.x - 10.0, left_rect.max.y - 10.0),
        );
        let mut l_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(left_content)
                .layout(egui::Layout::top_down(egui::Align::LEFT)),
        );

        // "All episodes" button (simulating a season selector)
        let all_btn = egui::Button::new(
            egui::RichText::new("Todos los episodios")
                .color(egui::Color32::WHITE)
                .size(sub_size)
                .strong(),
        )
        .fill(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 12))
        .corner_radius(6.0);

        l_ui.add_sized([left_panel_w - margin_x, 36.0], all_btn);
        l_ui.add_space(8.0);

        l_ui.label(
            egui::RichText::new(format!("{} episodios", details.episodes.len()))
                .color(egui::Color32::from_rgb(120, 120, 140))
                .size(sub_size * 0.85),
        );

        // ── Right panel: episode list ─────────────────────────────────
        let mut r_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(right_rect)
                .layout(egui::Layout::top_down(egui::Align::LEFT)),
        );

        let ep_thumb_h = (body_height * 0.16).clamp(70.0, 110.0);
        let ep_thumb_w = ep_thumb_h * (16.0 / 9.0);
        let ep_spacing = 14.0;
        let ep_right_margin = 16.0;

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(&mut r_ui, |ui| {
                ui.add_space(16.0);

                // Heading
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new("Episodios")
                            .color(egui::Color32::WHITE)
                            .size(title_size * 0.65)
                            .strong(),
                    );
                });

                ui.add_space(12.0);

                for (idx, ep) in details.episodes.iter().enumerate() {
                    let ep_num_str = ep.episode.to_string();
                    let row_height = ep_thumb_h + ep_spacing;

                    let (row_rect, row_resp) = ui.allocate_exact_size(
                        egui::vec2(right_rect.width() - ep_right_margin, row_height),
                        egui::Sense::click(),
                    );

                    // Hover highlight
                    if row_resp.hovered() {
                        ui.painter().rect_filled(
                            row_rect,
                            6.0,
                            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 10),
                        );
                    }

                    if row_resp.clicked() {
                        let anime_key = self.anime_key(details);
                        let number = Self::episode_number(ep, idx);
                        self.mark_episode_started(&anime_key, number);
                        println!("Reproducir episodio {}: {}", ep_num_str, ep.url);
                    }

                    // Thumbnail area
                    let thumb_rect = egui::Rect::from_min_size(
                        egui::pos2(row_rect.min.x + 8.0, row_rect.min.y + ep_spacing * 0.5),
                        egui::vec2(ep_thumb_w, ep_thumb_h),
                    );

                    if let Some(img) = &ep.image {
                        let image = egui::Image::new(img)
                            .fit_to_exact_size(thumb_rect.size())
                            .corner_radius(6.0);
                        image.paint_at(ui, thumb_rect);
                    } else {
                        // Placeholder thumbnail with episode number
                        ui.painter().rect_filled(
                            thumb_rect,
                            6.0,
                            egui::Color32::from_rgb(30, 30, 40),
                        );
                        ui.painter().text(
                            thumb_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            format!("E{}", ep_num_str),
                            egui::FontId::proportional(sub_size * 1.2),
                            egui::Color32::from_rgb(100, 100, 120),
                        );
                    }

                    // Episode info text beside the thumbnail
                    let text_x = thumb_rect.max.x + 16.0;
                    let text_rect = egui::Rect::from_min_max(
                        egui::pos2(text_x, thumb_rect.min.y + 4.0),
                        egui::pos2(row_rect.max.x - 8.0, thumb_rect.max.y),
                    );
                    let mut t_ui = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(text_rect)
                            .layout(egui::Layout::top_down(egui::Align::LEFT)),
                    );

                    t_ui.label(
                        egui::RichText::new(format!("Episodio {}", ep_num_str))
                            .color(egui::Color32::WHITE)
                            .size(sub_size * 1.1)
                            .strong(),
                    );

                    t_ui.add_space(4.0);

                    // Play icon hint
                    t_ui.label(
                        egui::RichText::new("▶ Reproducir")
                            .color(egui::Color32::from_rgb(120, 120, 145))
                            .size(sub_size * 0.85),
                    );
                }

                ui.add_space(30.0);
            });
    }

    // ═══════════════════════════════════════════════════════════════════════════
    //  HOME VIEW (existing)
    // ═══════════════════════════════════════════════════════════════════════════

    fn render_main_content(&mut self, ui: &mut egui::Ui) {
        if self.is_loading {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return;
        }

        if let Some(err) = self.error_msg.clone() {
            ui.centered_and_justified(|ui| {
                ui.colored_label(egui::Color32::RED, format!("Ocurrió un error: {}", err));
                if ui.button("Reintentar").clicked() {
                    self.load_latest(ui.ctx().clone());
                }
            });
            return;
        }

        // Clone items to avoid borrow conflicts with self inside closures
        let items = match self.items.clone() {
            Some(items) if !items.is_empty() => items,
            Some(_) => {
                ui.label("No se encontraron animes.");
                return;
            }
            None => return,
        };

        if self.focused_index >= items.len() {
            self.focused_index = 0;
        }
        let hero = items[self.focused_index].clone();

        if ui.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
            if self.focused_index < items.len().saturating_sub(1) {
                self.focused_index += 1;
            }
        }
        if ui.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
            if self.focused_index > 0 {
                self.focused_index -= 1;
            }
        }
        if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            let ctx = ui.ctx().clone();
            self.navigate_to_detail(&hero, ctx);
            return;
        }

        // Dimensiones adaptables según el tamaño de la ventana/pantalla
        let avail_width = ui.available_width();
        let avail_height = ui.available_height();

        // El banner ocupa TODA la altura disponible
        let hero_height = avail_height;
        let margin_x = (avail_width * 0.035).clamp(24.0, 52.0);

        // Tamaños de fuente responsivos
        let title_size = (avail_width * 0.026).clamp(22.0, 42.0);
        let subtitle_size = (title_size * 0.42).clamp(13.0, 16.0);
        let btn_font_size = (title_size * 0.44).clamp(14.0, 18.0);
        let btn_width = (avail_width * 0.14).clamp(160.0, 220.0);
        let btn_height = (hero_height * 0.06).clamp(38.0, 48.0);

        // Dimensiones dinámicas de las tarjetas del carrusel (16:9)
        let card_height = (avail_height * 0.22).clamp(130.0, 210.0);
        let card_width = card_height * (16.0 / 9.0);
        let card_spacing = (avail_width * 0.012).clamp(12.0, 22.0);

        // Deferred navigation: collect intent inside closures, execute after
        let mut nav_target: Option<LatestItem> = None;

        // Allocate full area for hero (no scroll – everything is overlaid)
        let (rect, _response) =
            ui.allocate_exact_size(egui::vec2(avail_width, hero_height), egui::Sense::hover());

        // Fetch TMDB backdrop si no lo tenemos aún
        let mut display_url = hero.image.clone();
        if let Some(cached_url) = self.tmdb_cache.get(&hero.title) {
            if !cached_url.is_empty() {
                display_url = Some(cached_url.clone());
            }
        } else if !self.tmdb_requested.contains(&hero.title) {
            self.tmdb_requested.insert(hero.title.clone());
            let title = hero.title.clone();
            let tx_tmdb = self.tx_tmdb.clone();
            let ctx = ui.ctx().clone();

            self.rt.spawn(async move {
                let found = Self::fetch_tmdb_backdrop(&title).await;
                if let Some(u) = found {
                    let _ = tx_tmdb.send((title, u));
                    ctx.request_repaint();
                } else {
                    let _ = tx_tmdb.send((title, "".to_string()));
                }
            });
        }

        // Fondo Hero – ocupa todo el viewport
        if let Some(img_url) = &display_url {
            let image = egui::Image::new(img_url).fit_to_exact_size(rect.size());
            image.paint_at(ui, rect);
        }

        let bg_color = ui.visuals().panel_fill;

        // 1. Viñeta lateral izquierda oscura
        let left_vignette_rect = egui::Rect::from_min_max(
            rect.min,
            egui::pos2(rect.min.x + (avail_width * 0.65), rect.max.y),
        );
        let left_color =
            egui::Color32::from_rgba_unmultiplied(bg_color.r(), bg_color.g(), bg_color.b(), 240);
        let transparent_bg =
            egui::Color32::from_rgba_unmultiplied(bg_color.r(), bg_color.g(), bg_color.b(), 0);
        Self::draw_gradient_rect(
            ui,
            left_vignette_rect,
            left_color,
            transparent_bg,
            left_color,
            transparent_bg,
        );

        // 2. Degradado inferior MUY OSCURO – cubre el ~55% inferior del banner
        //    para que el row de capítulos se vea bien
        let bottom_fade_rect = egui::Rect::from_min_max(
            egui::pos2(rect.min.x, rect.min.y + (hero_height * 0.55)),
            rect.max,
        );
        let bg_opaque =
            egui::Color32::from_rgba_unmultiplied(bg_color.r(), bg_color.g(), bg_color.b(), 255);
        Self::draw_gradient_rect(
            ui,
            bottom_fade_rect,
            transparent_bg,
            transparent_bg,
            bg_opaque,
            bg_opaque,
        );

        // === Contenido Hero (Episodio -> Título -> Botones) ===
        // Posicionar en la zona media-inferior, por encima del carrusel
        let carousel_zone_height = card_height + 55.0 + 50.0 + 45.0; // carrusel + label + espacios
        let hero_content_height = subtitle_size + title_size + btn_height * 2.0 + 60.0;
        let hero_content_y = rect.max.y - carousel_zone_height - hero_content_height - 10.0;
        let hero_content_rect = egui::Rect::from_min_max(
            egui::pos2(rect.min.x + margin_x, hero_content_y),
            egui::pos2(
                rect.max.x - margin_x,
                rect.max.y - carousel_zone_height - 10.0,
            ),
        );

        let mut ui_hero = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(hero_content_rect)
                .layout(egui::Layout::top_down(egui::Align::LEFT)),
        );

        // Episodio formateado limpiamente
        let ep_raw = hero.episode.as_deref().unwrap_or("1");
        let ep_label = if ep_raw.to_lowercase().contains("episodio") {
            ep_raw.to_uppercase()
        } else {
            format!("EPISODIO {}", ep_raw)
        };

        ui_hero.label(
            egui::RichText::new(ep_label)
                .color(egui::Color32::from_rgb(200, 200, 220))
                .size(subtitle_size)
                .strong(),
        );

        ui_hero.add_space(6.0);

        // Título del anime
        ui_hero.label(
            egui::RichText::new(&hero.title)
                .color(egui::Color32::WHITE)
                .size(title_size)
                .strong(),
        );

        ui_hero.add_space(14.0);

        // Botones Reproducir + Ver Detalles (en fila)
        ui_hero.horizontal(|ui| {
            let btn = egui::Button::new(
                egui::RichText::new("▶  Reproducir")
                    .color(egui::Color32::BLACK)
                    .size(btn_font_size)
                    .strong(),
            )
            .fill(egui::Color32::WHITE)
            .corner_radius(8.0);

            if ui.add_sized([btn_width, btn_height], btn).clicked() {
                println!("Reproducir: {}", hero.url);
            }

            // Botón Ver Detalles (solo icono de tres puntos)
            ui.add_space(8.0);
            let detail_btn = egui::Button::new(egui::RichText::new("").size(btn_font_size))
                .fill(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 15))
                .corner_radius(8.0);

            let detail_resp = ui.add_sized([btn_height, btn_height], detail_btn);
            if detail_resp.clicked() {
                nav_target = Some(hero.clone());
            }

            // Tres puntos dibujados (el glifo ⋮ no existe en las fuentes por defecto)
            let dot_r = (btn_height * 0.055).clamp(2.0, 3.0);
            let dot_gap = dot_r * 2.8;
            let dot_center = detail_resp.rect.center();
            for offset in [-1.0, 0.0, 1.0] {
                ui.painter().circle_filled(
                    egui::pos2(dot_center.x, dot_center.y + offset * dot_gap),
                    dot_r,
                    egui::Color32::WHITE,
                );
            }
        });

        // === Carrusel de capítulos superpuesto en la parte inferior del banner ===
        let carousel_top = rect.max.y - carousel_zone_height + 5.0;

        // Label "Últimos Episodios"
        let label_rect = egui::Rect::from_min_size(
            egui::pos2(rect.min.x + margin_x, carousel_top),
            egui::vec2(avail_width - margin_x * 2.0, 28.0),
        );
        let mut label_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(label_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        label_ui.label(
            egui::RichText::new("Últimos Episodios")
                .color(egui::Color32::WHITE)
                .size((title_size * 0.55).clamp(17.0, 24.0))
                .strong(),
        );

        // Animación suave de deslizamiento
        let focus_x_offset = margin_x;
        let animated_idx = ui.ctx().animate_value_with_time(
            ui.id().with("focus_anim"),
            self.focused_index as f32,
            0.22,
        );

        if (animated_idx - self.focused_index as f32).abs() > 0.001 {
            ui.ctx().request_repaint();
        }

        let strip_top = carousel_top + 36.0;
        let strip_height = card_height + 55.0;
        let strip_rect = egui::Rect::from_min_size(
            egui::pos2(rect.min.x, strip_top),
            egui::vec2(avail_width, strip_height),
        );

        for (idx, item) in items.iter().enumerate() {
            let card_x = strip_rect.min.x
                + focus_x_offset
                + ((idx as f32 - animated_idx) * (card_width + card_spacing));
            let is_focused = idx == self.focused_index;

            // Culling: saltar si está fuera de pantalla
            if card_x + card_width < strip_rect.min.x || card_x > strip_rect.max.x {
                continue;
            }

            // Cuadro de la tarjeta
            let card_rect = egui::Rect::from_min_size(
                egui::pos2(card_x, strip_rect.min.y + 4.0),
                egui::vec2(card_width, card_height),
            );

            let response = ui.interact(
                card_rect,
                ui.id().with("card").with(idx),
                egui::Sense::click(),
            );

            if response.clicked() {
                self.focused_index = idx;
            }

            if response.double_clicked() {
                nav_target = Some(item.clone());
            }

            if let Some(img_url) = &item.image {
                let image = egui::Image::new(img_url)
                    .fit_to_exact_size(card_rect.size())
                    .corner_radius(8.0);
                image.paint_at(ui, card_rect);
            }

            if is_focused {
                ui.painter().rect_stroke(
                    card_rect,
                    8.0,
                    egui::Stroke::new(3.5, egui::Color32::WHITE),
                    egui::StrokeKind::Outside,
                );
            } else if response.hovered() {
                ui.painter().rect_stroke(
                    card_rect,
                    8.0,
                    egui::Stroke::new(2.0, egui::Color32::from_rgb(180, 180, 190)),
                    egui::StrokeKind::Outside,
                );
            }

            // Texto debajo de la tarjeta
            let text_rect = egui::Rect::from_min_size(
                egui::pos2(card_x, card_rect.max.y + 8.0),
                egui::vec2(card_width, 42.0),
            );
            let mut text_ui = ui.new_child(egui::UiBuilder::new().max_rect(text_rect));
            text_ui.label(
                egui::RichText::new(&item.title)
                    .color(if is_focused {
                        egui::Color32::WHITE
                    } else {
                        egui::Color32::from_rgb(160, 160, 170)
                    })
                    .size((subtitle_size * 0.95).clamp(12.0, 15.0))
                    .strong(),
            );
            text_ui.label(
                egui::RichText::new(format!(
                    "Episodio {}",
                    item.episode.as_deref().unwrap_or("1")
                ))
                .color(if is_focused {
                    egui::Color32::LIGHT_GRAY
                } else {
                    egui::Color32::DARK_GRAY
                })
                .size((subtitle_size * 0.85).clamp(11.0, 13.0)),
            );
        }

        // Execute deferred navigation after closures
        if let Some(item) = nav_target {
            let ctx = ui.ctx().clone();
            self.navigate_to_detail(&item, ctx);
        }
    }

    // ═══════════════════════════════════════════════════════════════════════════
    //  Utility: draw a gradient rectangle with 4-corner colors
    // ═══════════════════════════════════════════════════════════════════════════

    fn draw_gradient_rect(
        ui: &mut egui::Ui,
        rect: egui::Rect,
        top_left: egui::Color32,
        top_right: egui::Color32,
        bottom_left: egui::Color32,
        bottom_right: egui::Color32,
    ) {
        let mut mesh = egui::Mesh::default();
        let idx = mesh.vertices.len() as u32;

        mesh.vertices.push(egui::epaint::Vertex {
            pos: rect.min,
            uv: egui::epaint::WHITE_UV,
            color: top_left,
        });
        mesh.vertices.push(egui::epaint::Vertex {
            pos: egui::pos2(rect.max.x, rect.min.y),
            uv: egui::epaint::WHITE_UV,
            color: top_right,
        });
        mesh.vertices.push(egui::epaint::Vertex {
            pos: rect.max,
            uv: egui::epaint::WHITE_UV,
            color: bottom_right,
        });
        mesh.vertices.push(egui::epaint::Vertex {
            pos: egui::pos2(rect.min.x, rect.max.y),
            uv: egui::epaint::WHITE_UV,
            color: bottom_left,
        });

        mesh.add_triangle(idx, idx + 1, idx + 2);
        mesh.add_triangle(idx, idx + 2, idx + 3);
        ui.painter().add(egui::Shape::mesh(mesh));
    }
}

impl eframe::App for AniGpuApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if let Ok(result) = self.rx.try_recv() {
            self.is_loading = false;
            match result {
                Ok(items) => self.items = Some(items),
                Err(e) => self.error_msg = Some(e),
            }
        }

        // Procesar nuevos fondos de TMDB
        let mut tmdb_changed = false;
        while let Ok((title, url)) = self.rx_tmdb.try_recv() {
            self.tmdb_cache.insert(title, url);
            tmdb_changed = true;
        }
        if tmdb_changed {
            self.save_tmdb_cache();
        }

        let sidebar_width = 58.0;
        let total_rect = ui.available_rect_before_wrap();

        let sidebar_rect = egui::Rect::from_min_size(
            total_rect.min,
            egui::vec2(sidebar_width, total_rect.height()),
        );
        let content_rect = egui::Rect::from_min_max(
            egui::pos2(sidebar_rect.max.x, total_rect.min.y),
            total_rect.max,
        );

        // 1. Renderizar Contenido Principal (con clip_rect activo para recortar a la izquierda)
        let mut content_ui = ui.new_child(egui::UiBuilder::new().max_rect(content_rect));
        content_ui.set_clip_rect(content_rect);

        match self.screen.clone() {
            Screen::Home => {
                self.render_main_content(&mut content_ui);
            }
            Screen::Search => {
                self.render_search_view(&mut content_ui);
            }
            Screen::Detail { .. } => {
                self.render_detail_view(&mut content_ui);
            }
        }

        // 2. Renderizar Barra Lateral DESPUÉS (Z-index superior para estar siempre al frente)
        ui.painter()
            .rect_filled(sidebar_rect, 0.0, egui::Color32::from_rgb(10, 10, 13));
        ui.painter().line_segment(
            [sidebar_rect.right_top(), sidebar_rect.right_bottom()],
            egui::Stroke::new(
                1.0,
                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 12),
            ),
        );

        let mut sidebar_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(sidebar_rect)
                .layout(egui::Layout::top_down(egui::Align::Center)),
        );
        self.render_sidebar(&mut sidebar_ui);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anigpu_core::models::EpisodeNum;

    fn details_with_episodes(count: u32) -> AnimeDetails {
        AnimeDetails {
            title: "Anime de prueba".to_string(),
            synopsis: String::new(),
            cover: None,
            backdrop: None,
            status: None,
            genres: Vec::new(),
            related: Vec::new(),
            episodes: (1..=count)
                .map(|n| EpisodeItem {
                    episode: EpisodeNum::Int(n),
                    url: format!("https://example.test/ep{}", n),
                    image: None,
                })
                .collect(),
        }
    }

    fn progress(completed: &[i64], current: Option<i64>) -> WatchProgress {
        WatchProgress {
            completed: completed.to_vec(),
            current: current.map(|ep| CurrentEpisode { ep, position: 0.0 }),
        }
    }

    #[test]
    fn sin_progreso_reproduce_el_capitulo_1() {
        let details = details_with_episodes(12);
        let (label, idx) = AniGpuApp::play_button_state(None, &details);
        assert_eq!(label, "▶  Reproducir - Capítulo 1");
        assert_eq!(idx, Some(0));
    }

    #[test]
    fn siete_completos_sigue_el_capitulo_8() {
        let details = details_with_episodes(12);
        let p = progress(&[1, 2, 3, 4, 5, 6, 7], None);
        let (label, idx) = AniGpuApp::play_button_state(Some(&p), &details);
        assert_eq!(label, "▶  Reproducir - Capítulo 8");
        assert_eq!(idx, Some(7));
    }

    #[test]
    fn capitulo_incompleto_pide_continuar() {
        let details = details_with_episodes(12);
        let p = progress(&[1, 2, 3, 4, 5, 6], Some(7));
        let (label, idx) = AniGpuApp::play_button_state(Some(&p), &details);
        assert_eq!(label, "▶  Continuar viendo - Capítulo 7");
        assert_eq!(idx, Some(6));
    }

    #[test]
    fn ultimo_completado_no_desborda_de_la_lista() {
        let details = details_with_episodes(3);
        let p = progress(&[1, 2, 3], None);
        let (label, idx) = AniGpuApp::play_button_state(Some(&p), &details);
        assert_eq!(label, "▶  Reproducir - Capítulo 3");
        assert_eq!(idx, Some(2));
    }

    #[test]
    fn sin_episodios_no_propone_nada() {
        let details = details_with_episodes(0);
        let (label, idx) = AniGpuApp::play_button_state(None, &details);
        assert_eq!(label, "▶  Reproducir");
        assert_eq!(idx, None);
    }

    #[test]
    fn episodios_sin_numero_usan_su_posicion() {
        let mut details = details_with_episodes(0);
        details.episodes.push(EpisodeItem {
            episode: EpisodeNum::Str("FINAL".to_string()),
            url: "https://example.test/final".to_string(),
            image: None,
        });
        let p = progress(&[], Some(1));
        let (label, idx) = AniGpuApp::play_button_state(Some(&p), &details);
        assert_eq!(label, "▶  Continuar viendo - Capítulo FINAL");
        assert_eq!(idx, Some(0));
    }

    #[test]
    fn portada_se_deriva_de_la_miniatura() {
        assert_eq!(
            cover_from_thumb("https://cdn.animeav1.com/thumbnails/4412.jpg"),
            Some("https://cdn.animeav1.com/covers/4412.jpg".to_string())
        );
    }

    #[test]
    fn sin_miniatura_no_hay_portada() {
        assert_eq!(cover_from_thumb("https://cdn.animeav1.com/covers/4412.jpg"), None);
        assert_eq!(cover_from_thumb("https://example.test/img.jpg"), None);
    }
}
