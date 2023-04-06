use anyhow::anyhow;
use eframe::{
    egui, emath,
    epaint::{Color32, Pos2, Rect, Rounding, Shape, Vec2},
};
use sd_core::{graph::HyperGraph, monoidal::MonoidalGraph};
use tracing::{debug_span, event, Level};

use crate::{highlighter::Highlighter, layout::Layouter, parser::Parser};

pub struct App {
    code: String,
    hypergraph: HyperGraph,
    monoidal_term: MonoidalGraph,
}

impl App {
    /// Called once before the first frame.
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        // if let Some(storage) = cc.storage {
        //     return eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
        // }

        App {
            code: Default::default(),
            hypergraph: Default::default(),
            monoidal_term: Default::default(),
        }
    }

    fn code_ui(&mut self, ui: &mut egui::Ui) {
        let mut layouter = |ui: &egui::Ui, source: &str, wrap_width: f32| {
            let mut layout_job = Highlighter::highlight(ui.ctx(), source);
            layout_job.wrap.max_width = wrap_width;
            ui.fonts(|f| f.layout_job(layout_job))
        };
        let text_edit_out = egui::TextEdit::multiline(&mut self.code)
            .code_editor()
            .layouter(&mut layouter)
            .show(ui);
        debug_span!("Processing graph");
        if text_edit_out.response.changed() {
            event!(Level::DEBUG, "Reparsing");
            let block = |app: &mut App| -> anyhow::Result<()> {
                let parse = Parser::parse(ui.ctx(), &app.code);
                let expr = parse.as_ref().as_ref().map_err(|e| anyhow!("{:?}", e))?;
                event!(Level::DEBUG, "Converting to hypergraph");
                app.hypergraph = expr.to_hypergraph()?;
                event!(Level::DEBUG, "Converting to monoidal term");
                app.monoidal_term = MonoidalGraph::from_hypergraph(&app.hypergraph)?;
                Ok(())
            };
            if block(self).is_err() {
                // Display error to user?
            }
        }
    }

    fn graph_ui(&mut self, ui: &mut egui::Ui) {
        let (response, painter) = ui.allocate_painter(
            Vec2::new(ui.available_width(), ui.available_height()),
            egui::Sense::drag(),
        );
        let to_screen = emath::RectTransform::from_to(
            Rect::from_min_size(Pos2::ZERO, response.rect.size()),
            response.rect,
        );

        // Background
        painter.add(Shape::rect_filled(
            response.rect,
            Rounding::none(),
            Color32::WHITE,
        ));
        let layout = Layouter::layout(ui.ctx(), &self.monoidal_term).unwrap();
        painter.extend(ui.fonts(|fonts| {
            sd_graphics::render::render(
                &layout,
                &self.monoidal_term,
                fonts,
                response.rect.size(),
                to_screen,
            )
        }));
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.columns(2, |columns| {
                egui::ScrollArea::vertical()
                    .id_source("code")
                    .show(&mut columns[0], |ui| self.code_ui(ui));
                egui::ScrollArea::both()
                    .id_source("graph")
                    .show(&mut columns[1], |ui| self.graph_ui(ui));
            })
        });
    }
}
