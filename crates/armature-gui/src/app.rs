//! egui application shell (feature `app`).
//!
//! Loads a binary (path supplied as the first CLI argument), runs the analysis
//! pipeline, and tiles three views: Hex, Assembly, and Graph. This is the
//! standalone rendering target; the production shell is `tpt-appfront`, which is
//! itself built on egui.

use armature_analysis::Analysis;
use eframe::egui;
use std::path::PathBuf;

/// Run the native egui application.
pub fn run() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "TPT Armature",
        options,
        Box::new(|_cc| Ok(Box::new(ArmatureApp::new()))),
    )
}

struct ArmatureApp {
    analysis: Option<Analysis>,
    status: String,
    selected: usize,
}

impl ArmatureApp {
    fn new() -> Self {
        let mut app = ArmatureApp {
            analysis: None,
            status: String::new(),
            selected: 0,
        };
        if let Some(path) = std::env::args().nth(1) {
            app.load(PathBuf::from(path));
        } else {
            app.status = "No binary supplied. Run: armature-gui <path-to-binary>".to_string();
        }
        app
    }

    fn load(&mut self, path: PathBuf) {
        match std::fs::read(&path) {
            Ok(bytes) => match armature_analysis::analyze_binary(&bytes) {
                Ok(analysis) => {
                    self.status = format!(
                        "{} {} | {} instructions | {}",
                        analysis.map.format,
                        analysis.map.arch,
                        analysis.instructions.len(),
                        analysis.cfg.summary()
                    );
                    self.analysis = Some(analysis);
                }
                Err(e) => self.status = format!("analysis failed: {e}"),
            },
            Err(e) => self.status = format!("cannot read {}: {e}", path.display()),
        }
    }
}

impl eframe::App for ArmatureApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("status").show(ctx, |ui| {
            ui.label(&self.status);
        });

        egui::CentralPanel::default().show(ctx, |ui| match &self.analysis {
            Some(analysis) => {
                ui.columns(3, |columns| {
                    render_hex(&mut columns[0], analysis);
                    render_asm(&mut columns[1], analysis, &mut self.selected);
                    render_graph(&mut columns[2], analysis);
                });
            }
            None => {
                ui.label(&self.status);
            }
        });
    }
}

fn render_hex(ui: &mut egui::Ui, analysis: &Analysis) {
    ui.heading("Hex");
    if let Some(section) = analysis.code_section() {
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (i, chunk) in section.data.chunks(16).enumerate() {
                let addr = section.virt_addr + (i * 16) as u64;
                let hex = chunk
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                ui.monospace(format!("{addr:08x}  {hex}"));
            }
        });
    }
}

fn render_asm(ui: &mut egui::Ui, analysis: &Analysis, selected: &mut usize) {
    ui.heading("Assembly");
    egui::ScrollArea::vertical().show(ui, |ui| {
        for (i, ins) in analysis.instructions.iter().enumerate() {
            let label = format!("0x{:08x}  {}", ins.address, ins.text);
            if ui.selectable_label(*selected == i, label).clicked() {
                *selected = i;
            }
        }
    });
}

fn render_graph(ui: &mut egui::Ui, analysis: &Analysis) {
    ui.heading("Graph");
    ui.label(analysis.cfg.summary());
    egui::ScrollArea::vertical().show(ui, |ui| {
        for e in &analysis.cfg.edges {
            let from = analysis.cfg.nodes[e.from].start;
            ui.label(format!(
                "0x{:x} --[{}]--> 0x{:x}",
                from, e.kind, analysis.cfg.nodes[e.to].start
            ));
        }
    });
}
