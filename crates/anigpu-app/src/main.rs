use eframe::egui;
use anigpu_core::models::LatestItem;
use anigpu_core::sources::get_source;
use std::sync::mpsc::{channel, Receiver, Sender};
use tokio::runtime::Runtime;
use std::sync::Arc;

fn main() -> eframe::Result<()> {
    // Inicializamos un "Runtime" de tokio para poder ejecutar funciones asíncronas
    // (como llamadas de red a las páginas web) en paralelo a la interfaz gráfica.
    let rt = Arc::new(Runtime::new().expect("Failed to create Tokio runtime"));

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_title("AniGPU Desktop"),
        ..Default::default()
    };

    eframe::run_native(
        "AniGPU",
        options,
        Box::new(move |_cc| {
            Ok(Box::new(AniGpuApp::new(rt)))
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
}

impl AniGpuApp {
    fn new(rt: Arc<Runtime>) -> Self {
        let (tx, rx) = channel();
        Self {
            rt,
            items: None,
            is_loading: false,
            error_msg: None,
            rx,
            tx,
        }
    }

    fn load_latest(&mut self, ctx: egui::Context) {
        self.is_loading = true;
        self.error_msg = None;
        let tx = self.tx.clone();
        
        // Lanzamos la tarea pesada (el web scraper) en segundo plano
        self.rt.spawn(async move {
            let source = get_source(Some("animeav1")); // Cambiamos a animeav1 para evitar el bloqueo 403
            match source.get_latest().await {
                Ok(items) => {
                    let _ = tx.send(Ok(items));
                }
                Err(e) => {
                    let _ = tx.send(Err(e.to_string()));
                }
            }
            // Le pedimos a la ventana que se vuelva a dibujar para mostrar los resultados
            ctx.request_repaint();
        });
    }
}

impl eframe::App for AniGpuApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Revisamos si la tarea en segundo plano ya terminó y nos mandó los resultados
        if let Ok(result) = self.rx.try_recv() {
            self.is_loading = false;
            match result {
                Ok(items) => self.items = Some(items),
                Err(e) => self.error_msg = Some(e),
            }
        }

        ui.heading("AniGPU - Explorador de Anime");
        ui.separator();

        ui.horizontal(|ui| {
            if ui.button("Cargar / Refrescar Cartelera").clicked() && !self.is_loading {
                self.load_latest(ui.ctx().clone());
            }
            if self.is_loading {
                ui.spinner();
                ui.label("Extrayendo episodios recientes de animeav1...");
            }
        });

        ui.separator();

        if let Some(ref err) = self.error_msg {
            ui.colored_label(egui::Color32::RED, format!("Ocurrió un error: {}", err));
        }

        // Mostrar la lista de animes
        if let Some(ref items) = self.items {
            egui::ScrollArea::vertical().show(ui, |ui| {
                for item in items {
                    ui.group(|ui| {
                        ui.label(egui::RichText::new(&item.title).strong().size(18.0));
                        ui.label(format!("Episodio: {}", item.episode.as_deref().unwrap_or("N/A")));
                        ui.hyperlink_to("Ver en navegador", &item.url);
                    });
                    ui.add_space(8.0);
                }
            });
        } else if !self.is_loading && self.error_msg.is_none() {
            ui.label("Haz clic en 'Cargar' para obtener los últimos episodios usando el core.");
        }
    }
}
