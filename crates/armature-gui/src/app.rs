//! egui application shell (feature `app`).
//!
//! Loads a binary (path supplied as the first CLI argument), runs the analysis
//! pipeline, and tiles three views: Hex, Assembly, and Graph. This is the
//! standalone rendering target; the production shell is `tpt-appfront`, which is
//! itself built on egui.

use armature_analysis::{Analysis, XrefKind};
use armature_ir::{Module, Mnemonic, Operand};
use eframe::egui;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;

/// Maximum basic blocks drawn in the graph view (huge functions are capped).
const GRAPH_NODE_CAP: usize = 600;
const NODE_W: f32 = 170.0;
const NODE_H: f32 = 46.0;
const GAP_X: f32 = 70.0;
const GAP_Y: f32 = 26.0;

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
    selected_function: usize,
    /// Instruction virtual address -> index in `analysis.instructions`.
    addr_to_idx: HashMap<u64, usize>,
    /// When `Some`, the Assembly view should scroll to this index (set by
    /// graph-node / X-ref jumps) and then clear it.
    pending_scroll: Option<usize>,
}

impl ArmatureApp {
    fn new() -> Self {
        let mut app = ArmatureApp {
            analysis: None,
            status: String::new(),
            selected: 0,
            selected_function: 0,
            addr_to_idx: HashMap::new(),
            pending_scroll: None,
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
                    self.addr_to_idx = analysis
                        .instructions
                        .iter()
                        .enumerate()
                        .map(|(i, ins)| (ins.address, i))
                        .collect();
                    self.status = format!(
                        "{} {} | {} instructions | {} functions | {}",
                        analysis.map.format,
                        analysis.map.arch,
                        analysis.instructions.len(),
                        analysis.module.functions.len(),
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
                    render_asm(
                        &mut columns[1],
                        analysis,
                        &self.addr_to_idx,
                        &mut self.selected,
                        &mut self.pending_scroll,
                    );
                    render_graph(
                        &mut columns[2],
                        analysis,
                        &mut self.selected_function,
                        &self.addr_to_idx,
                        &mut self.selected,
                        &mut self.pending_scroll,
                    );
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

fn render_asm(
    ui: &mut egui::Ui,
    analysis: &Analysis,
    addr_to_idx: &HashMap<u64, usize>,
    selected: &mut usize,
    pending_scroll: &mut Option<usize>,
) {
    ui.heading("Assembly");
    if let Some(ins) = analysis.instructions.get(*selected) {
        ui.monospace(format!("0x{:08x}  {}", ins.address, ins.text));
        let incoming = analysis.xrefs.refs_to_addr(ins.address);
        if !incoming.is_empty() {
            ui.label(format!("{} xref(s) to this address:", incoming.len()));
            for x in incoming {
                let kind = match x.kind {
                    XrefKind::Code => "code",
                    XrefKind::Symbol => "symbol",
                };
                let label = format!("  from 0x{:08x} [{}]", x.from, kind);
                if ui.selectable_label(false, label).clicked() {
                    if let Some(&idx) = addr_to_idx.get(&x.from) {
                        *selected = idx;
                        *pending_scroll = Some(idx);
                    }
                }
            }
        }
        if matches!(ins.mnemonic, Mnemonic::Call | Mnemonic::Jmp | Mnemonic::Jcc(_)) {
            if let Some(target) = ins
                .operands
                .iter()
                .find_map(|o| if let Operand::Imm(v) = o { Some(*v) } else { None })
            {
                if let Some(&idx) = addr_to_idx.get(&target) {
                    let label = format!("  -> 0x{:08x}", target);
                    if ui.selectable_label(false, label).clicked() {
                        *selected = idx;
                        *pending_scroll = Some(idx);
                    }
                }
            }
        }
        ui.separator();
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        for (i, ins) in analysis.instructions.iter().enumerate() {
            let label = format!("0x{:08x}  {}", ins.address, ins.text);
            let resp = ui.selectable_label(*selected == i, label);
            if resp.clicked() {
                *selected = i;
            }
            if *selected == i && *pending_scroll == Some(i) {
                resp.scroll_to_me(Some(egui::Align::Center));
                *pending_scroll = None;
            }
        }
    });
}

fn render_graph(
    ui: &mut egui::Ui,
    analysis: &Analysis,
    selected_function: &mut usize,
    addr_to_idx: &HashMap<u64, usize>,
    selected: &mut usize,
    pending_scroll: &mut Option<usize>,
) {
    ui.heading("Graph");
    let funcs = &analysis.module.functions;
    if funcs.is_empty() {
        ui.label("(no functions recovered)");
        return;
    }

    egui::ComboBox::from_label("Function").show_ui(ui, |ui| {
        for (i, f) in funcs.iter().enumerate() {
            let label = f
                .name
                .clone()
                .unwrap_or_else(|| format!("0x{:x}", f.start));
            ui.selectable_value(selected_function, i, label);
        }
    });

    let idx = (*selected_function).min(funcs.len() - 1);
    let func = &funcs[idx];
    let single = Module {
        functions: vec![func.clone()],
    };
    let cfg = armature_analysis::build_cfg(&single);
    ui.label(format!(
        "{} blocks, {} edges (function {}/{})",
        cfg.nodes.len(),
        cfg.edges.len(),
        idx + 1,
        funcs.len()
    ));

    let selected_addr = analysis.instructions.get(*selected).map(|i| i.address);
    draw_cfg_canvas(ui, &cfg, addr_to_idx, selected_addr, selected);
}

/// Draw one function's CFG as a layered node-and-edge canvas (not the raw edge
/// list). Nodes are placed in BFS-ranked columns; edges carry arrowheads;
/// clicking a node jumps the Assembly view to that block. The graph is
/// scrollable and capped at [`GRAPH_NODE_CAP`] blocks.
fn draw_cfg_canvas(
    ui: &mut egui::Ui,
    cfg: &armature_analysis::Cfg,
    addr_to_idx: &HashMap<u64, usize>,
    selected_addr: Option<u64>,
    selected: &mut usize,
) {
    let n = cfg.nodes.len();
    if n == 0 {
        return;
    }
    let capped = n.min(GRAPH_NODE_CAP);

    let mut rank = vec![-1i32; capped];
    if capped > 0 {
        rank[0] = 0;
    }
    let mut queue: VecDeque<usize> = VecDeque::new();
    queue.push_back(0);
    while let Some(u) = queue.pop_front() {
        for e in &cfg.edges {
            if e.from == u && e.to < capped && rank[e.to] < 0 {
                rank[e.to] = rank[u] + 1;
                queue.push_back(e.to);
            }
        }
    }

    let mut by_rank: HashMap<i32, Vec<usize>> = HashMap::new();
    for (i, &rval) in rank.iter().enumerate() {
        let r = if rval < 0 { i32::MAX } else { rval };
        by_rank.entry(r).or_default().push(i);
    }

    let mut pos: HashMap<usize, egui::Vec2> = HashMap::new();
    let mut max_x: f32 = 0.0;
    let mut max_y: f32 = 0.0;
    let mut ranks: Vec<i32> = by_rank.keys().copied().collect();
    ranks.sort_unstable();
    for r in ranks {
        let nodes = &by_rank[&r];
        for (j, &node) in nodes.iter().enumerate() {
            let x = r as f32 * (NODE_W + GAP_X);
            let y = j as f32 * (NODE_H + GAP_Y);
            pos.insert(node, egui::vec2(x, y));
            max_x = max_x.max(x + NODE_W);
            max_y = max_y.max(y + NODE_H);
        }
    }

    let total = egui::vec2(max_x + 20.0_f32, max_y + 20.0_f32);
    egui::ScrollArea::both().show(ui, |ui| {
        let _reserved = ui.allocate_space(total);
        let painter = ui.painter();
        let origin = egui::Pos2::ZERO;
        let edge = egui::Stroke::new(1.0_f32, egui::Color32::from_gray(150));

        for e in &cfg.edges {
            if e.from >= capped || e.to >= capped {
                continue;
            }
            let p1 = origin + pos[&e.from] + egui::vec2(NODE_W / 2.0_f32, NODE_H);
            let p2 = origin + pos[&e.to] + egui::vec2(NODE_W / 2.0_f32, 0.0_f32);
            painter.line_segment([p1, p2], edge);
            draw_arrowhead(painter, p1, p2, edge);
        }

        for i in 0..capped {
            let p = origin + pos[&i];
            let nrect = egui::Rect::from_min_size(p, egui::vec2(NODE_W, NODE_H));
            let is_selected = selected_addr == Some(cfg.nodes[i].start);
            painter.rect_filled(
                nrect,
                egui::Rounding::same(6.0_f32),
                egui::Color32::from_rgb(40, 48, 64),
            );
            painter.rect_stroke(
                nrect,
                egui::Rounding::same(6.0_f32),
                egui::Stroke::new(
                    if is_selected { 2.0_f32 } else { 1.0_f32 },
                    if is_selected {
                        egui::Color32::GOLD
                    } else {
                        egui::Color32::from_gray(110)
                    },
                ),
            );
            let first = cfg.nodes[i]
                .instructions
                .first()
                .map(|ins| ins.text.clone())
                .unwrap_or_default();
            let first = if first.chars().count() > 22 {
                let truncated: String = first.chars().take(22).collect();
                format!("{truncated}…")
            } else {
                first
            };
            painter.text(
                p + egui::vec2(6.0_f32, 4.0_f32),
                egui::Align2::LEFT_TOP,
                format!("0x{:x}", cfg.nodes[i].start),
                egui::FontId::monospace(10.0_f32),
                egui::Color32::GOLD,
            );
            painter.text(
                p + egui::vec2(6.0_f32, 18.0_f32),
                egui::Align2::LEFT_TOP,
                first,
                egui::FontId::monospace(10.0_f32),
                egui::Color32::WHITE,
            );

            let resp = ui.interact(nrect, egui::Id::new(("graph_node", i)), egui::Sense::click());
            if resp.clicked() {
                if let Some(&idx) = addr_to_idx.get(&cfg.nodes[i].start) {
                    *selected = idx;
                }
            }
        }
    });

    if n > capped {
        ui.label(format!("showing first {capped} of {n} blocks"));
    }
}

/// Draw a small arrowhead at `to` pointing along the `from`->`to` direction.
fn draw_arrowhead(
    painter: &egui::Painter,
    from: egui::Pos2,
    to: egui::Pos2,
    stroke: egui::Stroke,
) {
    let dir = to - from;
    let len = dir.length();
    if len <= 1.0 {
        return;
    }
    let u = dir / len;
    let perp = egui::vec2(-u.y, u.x);
    let size = 7.0_f32;
    let back = to - u * size;
    let left = back + perp * size * 0.5;
    let right = back - perp * size * 0.5;
    painter.line_segment([to, left], stroke);
    painter.line_segment([to, right], stroke);
}
