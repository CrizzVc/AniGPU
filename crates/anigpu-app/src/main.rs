use anigpu_core::models::LatestItem;
use anigpu_core::sources::get_source;
use eframe::egui;
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

struct AniGpuApp {
    rt: Arc<Runtime>,
    items: Option<Vec<LatestItem>>,
    is_loading: bool,
    error_msg: Option<String>,
    rx: Receiver<Result<Vec<LatestItem>, String>>,
    tx: Sender<Result<Vec<LatestItem>, String>>,
    focused_index: usize,
    selected_tab: usize,
    tmdb_cache: std::collections::HashMap<String, String>,
    rx_tmdb: Receiver<(String, String)>,
    tx_tmdb: Sender<(String, String)>,
    last_requested_title: String,
}

impl AniGpuApp {
    fn new(rt: Arc<Runtime>) -> Self {
        let (tx, rx) = channel();
        let (tx_tmdb, rx_tmdb) = channel();
        Self {
            rt,
            items: None,
            is_loading: true,
            error_msg: None,
            rx,
            tx,
            focused_index: 0,
            selected_tab: 0,
            tmdb_cache: std::collections::HashMap::new(),
            rx_tmdb,
            tx_tmdb,
            last_requested_title: String::new(),
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
                                        if let Ok(t_json) = t_resp.json::<serde_json::Value>().await {
                                            if let Some(t_results) = t_json["results"].as_array() {
                                                for res in t_results {
                                                    if let Some(path) = res["backdrop_path"].as_str() {
                                                        return Some(format!("https://image.tmdb.org/t/p/original{}", path));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            // Si TMDB aún no tiene backdrop, usar el coverImage oficial de Kitsu (alta resolución)
                            if let Some(cover_url) = attrs["coverImage"]["original"].as_str()
                                .or_else(|| attrs["coverImage"]["large"].as_str()) {
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

        if let Some(ref items) = self.items {
            if items.is_empty() {
                ui.label("No se encontraron animes.");
                return;
            }

            if self.focused_index >= items.len() {
                self.focused_index = 0;
            }
            let hero = &items[self.focused_index];

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
                println!("Reproducir (Enter): {}", hero.url);
            }

            // Dimensiones adaptables según el tamaño de la ventana/pantalla
            let avail_width = ui.available_width();
            let avail_height = ui.available_height();

            // Altura dinámica del hero más alta para lucir el arte (aprox 64% de la altura disponible)
            let hero_height = (avail_height * 0.64).clamp(360.0, 750.0);
            let margin_x = (avail_width * 0.035).clamp(24.0, 52.0);

            // Tamaños de fuente responsivos
            let title_size = (avail_width * 0.026).clamp(22.0, 42.0);
            let subtitle_size = (title_size * 0.42).clamp(13.0, 16.0);
            let btn_font_size = (title_size * 0.44).clamp(14.0, 18.0);
            let btn_width = (avail_width * 0.14).clamp(160.0, 220.0);
            let btn_height = (hero_height * 0.09).clamp(38.0, 48.0);

            // Dimensiones dinámicas de las tarjetas del carrusel (16:9)
            let card_height = (avail_height * 0.19).clamp(110.0, 175.0);
            let card_width = card_height * (16.0 / 9.0);
            let card_spacing = (avail_width * 0.012).clamp(12.0, 22.0);

            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    // --- Hero Section ---
                    let (rect, _response) = ui.allocate_exact_size(
                        egui::vec2(avail_width, hero_height),
                        egui::Sense::hover(),
                    );

                    // Fetch TMDB backdrop si no lo tenemos aún
                    let mut display_url = hero.image.clone();
                    if let Some(cached_url) = self.tmdb_cache.get(&hero.title) {
                        if !cached_url.is_empty() {
                            display_url = Some(cached_url.clone());
                        }
                    } else if self.last_requested_title != hero.title {
                        self.last_requested_title = hero.title.clone();
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

                    // Fondo Hero (ocupa todo el alto y ancho del contenedor rect)
                    if let Some(img_url) = &display_url {
                        let image = egui::Image::new(img_url).fit_to_exact_size(rect.size());
                        image.paint_at(ui, rect);
                    }

                    let bg_color = ui.visuals().panel_fill;

                    // 1. Viñeta lateral izquierda con el color exacto del fondo
                    let left_vignette_rect = egui::Rect::from_min_max(
                        rect.min,
                        egui::pos2(rect.min.x + (avail_width * 0.65), rect.max.y),
                    );
                    let mut left_mesh = egui::Mesh::default();
                    let left_color = egui::Color32::from_rgba_unmultiplied(
                        bg_color.r(),
                        bg_color.g(),
                        bg_color.b(),
                        230,
                    );
                    let transparent_bg = egui::Color32::from_rgba_unmultiplied(
                        bg_color.r(),
                        bg_color.g(),
                        bg_color.b(),
                        0,
                    );

                    let l_idx = left_mesh.vertices.len() as u32;
                    left_mesh.vertices.push(egui::epaint::Vertex {
                        pos: left_vignette_rect.min,
                        uv: egui::epaint::WHITE_UV,
                        color: left_color,
                    });
                    left_mesh.vertices.push(egui::epaint::Vertex {
                        pos: egui::pos2(left_vignette_rect.max.x, left_vignette_rect.min.y),
                        uv: egui::epaint::WHITE_UV,
                        color: transparent_bg,
                    });
                    left_mesh.vertices.push(egui::epaint::Vertex {
                        pos: egui::pos2(left_vignette_rect.max.x, left_vignette_rect.max.y),
                        uv: egui::epaint::WHITE_UV,
                        color: transparent_bg,
                    });
                    left_mesh.vertices.push(egui::epaint::Vertex {
                        pos: egui::pos2(left_vignette_rect.min.x, left_vignette_rect.max.y),
                        uv: egui::epaint::WHITE_UV,
                        color: left_color,
                    });
                    left_mesh.add_triangle(l_idx, l_idx + 1, l_idx + 2);
                    left_mesh.add_triangle(l_idx, l_idx + 2, l_idx + 3);
                    ui.painter().add(egui::Shape::mesh(left_mesh));

                    // 2. Gradiente inferior suave (difuminado hacia la base del contenedor)
                    let bottom_fade_rect = egui::Rect::from_min_max(
                        egui::pos2(rect.min.x, rect.min.y + (hero_height * 0.50)),
                        rect.max,
                    );
                    let mut btm_mesh = egui::Mesh::default();

                    let b_idx = btm_mesh.vertices.len() as u32;
                    btm_mesh.vertices.push(egui::epaint::Vertex {
                        pos: bottom_fade_rect.min,
                        uv: egui::epaint::WHITE_UV,
                        color: transparent_bg,
                    });
                    btm_mesh.vertices.push(egui::epaint::Vertex {
                        pos: egui::pos2(bottom_fade_rect.max.x, bottom_fade_rect.min.y),
                        uv: egui::epaint::WHITE_UV,
                        color: transparent_bg,
                    });
                    btm_mesh.vertices.push(egui::epaint::Vertex {
                        pos: bottom_fade_rect.max,
                        uv: egui::epaint::WHITE_UV,
                        color: bg_color,
                    });
                    btm_mesh.vertices.push(egui::epaint::Vertex {
                        pos: egui::pos2(bottom_fade_rect.min.x, bottom_fade_rect.max.y),
                        uv: egui::epaint::WHITE_UV,
                        color: bg_color,
                    });
                    btm_mesh.add_triangle(b_idx, b_idx + 1, b_idx + 2);
                    btm_mesh.add_triangle(b_idx, b_idx + 2, b_idx + 3);
                    ui.painter().add(egui::Shape::mesh(btm_mesh));

                    // --- Contenido Hero (Episodio -> Título -> Botón Reproducir en orden Top-Down) ---
                    let hero_content_height = subtitle_size + title_size + btn_height + 40.0;
                    let hero_content_y = rect.max.y - hero_content_height - 20.0;
                    let hero_content_rect = egui::Rect::from_min_max(
                        egui::pos2(rect.min.x + margin_x, hero_content_y),
                        egui::pos2(rect.max.x - margin_x, rect.max.y - 15.0),
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

                    // Botón Reproducir
                    let btn = egui::Button::new(
                        egui::RichText::new("▶  Reproducir")
                            .color(egui::Color32::BLACK)
                            .size(btn_font_size)
                            .strong(),
                    )
                    .fill(egui::Color32::WHITE)
                    .corner_radius(8.0);

                    if ui_hero.add_sized([btn_width, btn_height], btn).clicked() {
                        println!("Reproducir: {}", hero.url);
                    }

                    ui.add_space((avail_height * 0.025).clamp(12.0, 24.0));

                    // --- Sección Lista Horizontal / Carrusel ---
                    ui.horizontal(|ui| {
                        ui.add_space(margin_x);
                        ui.label(
                            egui::RichText::new("Últimos Episodios")
                                .color(egui::Color32::WHITE)
                                .size((title_size * 0.55).clamp(17.0, 24.0))
                                .strong(),
                        );
                    });

                    ui.add_space((avail_height * 0.015).clamp(8.0, 16.0));

                    let focus_x_offset = margin_x;

                    // Animación suave de deslizamiento
                    let animated_idx = ui.ctx().animate_value_with_time(
                        ui.id().with("focus_anim"),
                        self.focused_index as f32,
                        0.22,
                    );

                    if (animated_idx - self.focused_index as f32).abs() > 0.001 {
                        ui.ctx().request_repaint();
                    }

                    let strip_height = card_height + 55.0;
                    let (strip_rect, _resp) = ui.allocate_exact_size(
                        egui::vec2(avail_width, strip_height),
                        egui::Sense::hover(),
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
                            println!("Clicked: {}", item.url);
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
                });
        }
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
        while let Ok((title, url)) = self.rx_tmdb.try_recv() {
            self.tmdb_cache.insert(title, url);
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
        self.render_main_content(&mut content_ui);

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
