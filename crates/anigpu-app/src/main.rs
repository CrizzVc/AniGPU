use eframe::egui;
use anigpu_core::models::LatestItem;
use anigpu_core::sources::get_source;
use std::sync::mpsc::{channel, Receiver, Sender};
use tokio::runtime::Runtime;
use std::sync::Arc;

fn main() -> eframe::Result<()> {
    let rt = Arc::new(Runtime::new().expect("Failed to create Tokio runtime"));

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 700.0])
            .with_title("AniGPU"),
        ..Default::default()
    };

    eframe::run_native(
        "AniGPU",
        options,
        Box::new(move |cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            let mut visuals = egui::Visuals::dark();
            visuals.window_fill = egui::Color32::from_rgb(20, 20, 22);
            visuals.panel_fill = egui::Color32::from_rgb(18, 18, 20);
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
}

impl AniGpuApp {
    fn new(rt: Arc<Runtime>) -> Self {
        let (tx, rx) = channel();
        Self {
            rt,
            items: None,
            is_loading: true,
            error_msg: None,
            rx,
            tx,
            focused_index: 0,
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

            let hero = &items[0];

            if ui.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
                if self.focused_index < items.len().saturating_sub(2) {
                    self.focused_index += 1;
                }
            }
            if ui.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
                if self.focused_index > 0 {
                    self.focused_index -= 1;
                }
            }
            
            egui::ScrollArea::vertical().show(ui, |ui| {
                // Hero Section
                let hero_height = 400.0;
                let (rect, _response) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), hero_height),
                    egui::Sense::hover(),
                );
                
                // Draw Hero Background Image (we use an image URL if available, else a placeholder)
                if let Some(img_url) = &hero.image {
                    let image = egui::Image::new(img_url)
                        .fit_to_exact_size(rect.size())
                        .corner_radius(10.0);
                    image.paint_at(ui, rect);
                }
                
                // Gradient overlay
                // We could build a gradient mesh manually here, but for simplicity we'll just overlay a dark rectangle with some alpha.
                ui.painter().rect_filled(
                    rect,
                    10.0,
                    egui::Color32::from_black_alpha(150)
                );

                // Hero Text & Buttons
                let mut ui_hero = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::bottom_up(egui::Align::LEFT)));
                ui_hero.add_space(30.0); // Padding from bottom
                ui_hero.horizontal(|ui| {
                    ui.add_space(40.0);
                    ui.vertical(|ui| {
                        let btn = egui::Button::new(
                            egui::RichText::new("   ▶ Reproducir   ")
                                .color(egui::Color32::BLACK)
                                .size(18.0)
                                .strong()
                        )
                        .fill(egui::Color32::WHITE)
                        .corner_radius(8.0);
                        
                        if ui.add_sized([180.0, 45.0], btn).clicked() {
                            println!("Reproducir: {}", hero.url);
                        }
                    });
                });
                ui_hero.add_space(20.0);
                
                ui_hero.horizontal(|ui| {
                    ui.add_space(40.0);
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new(format!("Episodio {}", hero.episode.as_deref().unwrap_or("1")))
                                .color(egui::Color32::LIGHT_GRAY)
                                .size(16.0)
                        );
                        ui.add_space(5.0);
                        ui.label(
                            egui::RichText::new(&hero.title)
                                .color(egui::Color32::WHITE)
                                .size(42.0)
                                .strong()
                        );
                    });
                });

                ui.add_space(30.0);
                
                // Horizontal list section
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    ui.label(
                        egui::RichText::new("Últimos Episodios")
                            .color(egui::Color32::WHITE)
                            .size(22.0)
                            .strong()
                    );
                });
                
                ui.add_space(15.0);
                
                // Horizontal animated list section
                let card_width = 220.0;
                let card_height = 130.0;
                let spacing = 15.0;
                let focus_x_offset = 20.0; // Where the focused card starts from the left

                // Smooth sliding animation!
                let animated_idx = ui.ctx().animate_value_with_time(
                    ui.id().with("focus_anim"),
                    self.focused_index as f32,
                    0.25, // seconds for animation
                );
                
                // Repaint if the animation is still running
                if (animated_idx - self.focused_index as f32).abs() > 0.001 {
                    ui.ctx().request_repaint();
                }

                let strip_height = card_height + 60.0;
                let (strip_rect, _resp) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), strip_height),
                    egui::Sense::hover(),
                );

                for (idx, item) in items.iter().skip(1).enumerate() {
                    let card_x = strip_rect.min.x + focus_x_offset + ((idx as f32 - animated_idx) * (card_width + spacing));
                    let is_focused = idx == self.focused_index;

                    // Culling: skip drawing if entirely off-screen
                    if card_x + card_width < strip_rect.min.x || card_x > strip_rect.max.x {
                        continue;
                    }

                    // Card bounding box
                    let card_rect = egui::Rect::from_min_size(
                        egui::pos2(card_x, strip_rect.min.y + 10.0),
                        egui::vec2(card_width, card_height),
                    );

                    let response = ui.interact(card_rect, ui.id().with("card").with(idx), egui::Sense::click());

                    if let Some(img_url) = &item.image {
                        let image = egui::Image::new(img_url)
                            .fit_to_exact_size(card_rect.size())
                            .corner_radius(8.0);
                        image.paint_at(ui, card_rect);
                    }

                    if is_focused {
                        ui.painter().rect_stroke(card_rect, 8.0, egui::Stroke::new(4.0, egui::Color32::WHITE), egui::StrokeKind::Outside);
                    } else if response.hovered() {
                        ui.painter().rect_stroke(card_rect, 8.0, egui::Stroke::new(2.0, egui::Color32::LIGHT_GRAY), egui::StrokeKind::Outside);
                    }

                    // Text below the card
                    let text_rect = egui::Rect::from_min_size(
                        egui::pos2(card_x, card_rect.max.y + 10.0),
                        egui::vec2(card_width, 40.0),
                    );
                    let mut text_ui = ui.new_child(egui::UiBuilder::new().max_rect(text_rect));
                    text_ui.label(
                        egui::RichText::new(&item.title)
                            .color(if is_focused { egui::Color32::WHITE } else { egui::Color32::GRAY })
                            .size(15.0)
                            .strong()
                    );
                    text_ui.label(
                        egui::RichText::new(format!("Episodio {}", item.episode.as_deref().unwrap_or("1")))
                            .color(if is_focused { egui::Color32::LIGHT_GRAY } else { egui::Color32::DARK_GRAY })
                            .size(13.0)
                    );

                    if response.clicked() {
                        println!("Clicked: {}", item.url);
                    }
                }
                
                ui.add_space(40.0);
            });
        }
    }
}
