//! egui application shell (feature `app`).
//!
//! Loads a binary (path supplied as the first CLI argument), runs the analysis
//! pipeline on a background thread, and tiles three views: Hex, Assembly, and
//! Graph. This is the standalone rendering target; the production shell is
//! `tpt-appfront`, which is itself built on egui.

use eframe::egui;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use tpt_armature_analysis::{Analysis, Cfg, XrefKind};
use tpt_armature_ir::{Mnemonic, Module, Operand};

#[cfg(feature = "scripts")]
use tpt_armature_ext::{default_rename_script, ScriptHost};

/// Which sub-view the right-hand info panel is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InfoTab {
    Strings,
    Pseudocode,
}

/// Maximum basic blocks drawn in the graph view (huge functions are capped).
const GRAPH_NODE_CAP: usize = 600;
const NODE_W: f32 = 170.0;
const NODE_H: f32 = 46.0;
const GAP_X: f32 = 70.0;
const GAP_Y: f32 = 26.0;

/// Message produced by the background analysis loader.
enum LoadMsg {
    Done {
        analysis: Box<Analysis>,
        path: PathBuf,
    },
    Error(String),
}

/// Run the egui application. On native targets this opens an OS window; on
/// `wasm32` it mounts into the `#armature_canvas` HTML canvas element.
pub fn run() -> eframe::Result<()> {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;
        let canvas = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("armature_canvas"))
            .and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok())
            .expect("missing #armature_canvas element");
        let web_options = eframe::WebOptions::default();
        let runner = eframe::WebRunner::new();
        wasm_bindgen_futures::spawn_local(async move {
            if let Err(e) = runner
                .start(
                    canvas,
                    web_options,
                    Box::new(|_cc| Ok(Box::new(ArmatureApp::new()))),
                )
                .await
            {
                let msg = format!("tpt-armature gui failed to start: {e:?}");
                web_sys::console::error_1(&wasm_bindgen::JsValue::from_str(&msg));
            }
        });
        Ok(())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let options = eframe::NativeOptions::default();
        eframe::run_native(
            "TPT Armature",
            options,
            Box::new(|_cc| Ok(Box::new(ArmatureApp::new()))),
        )
    }
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
    /// Receiver for the background analysis loader. `Some` while a load is in
    /// flight; `None` once it has completed or errored.
    load_rx: Option<Receiver<LoadMsg>>,
    /// Per-function CFG cache, keyed by the selected function index. Rebuilt
    /// only when the selection changes (not every frame).
    cached_cfg: Option<(usize, Cfg)>,
    /// Script console source text (Rhai). Only populated when the `scripts`
    /// feature is enabled.
    #[cfg(feature = "scripts")]
    script_source: String,
    /// Script console output (renames + errors). Only populated when the
    /// `scripts` feature is enabled.
    #[cfg(feature = "scripts")]
    script_output: String,
    /// Goto-address text box contents (hex or decimal). Enter jumps the
    /// Assembly view to that instruction.
    goto_text: String,
    /// Search box contents; Enter jumps to the next matching instruction.
    search_text: String,
    /// Selected right-hand info panel tab.
    info_tab: InfoTab,
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
            load_rx: None,
            cached_cfg: None,
            #[cfg(feature = "scripts")]
            script_source: default_rename_script().to_string(),
            #[cfg(feature = "scripts")]
            script_output: String::new(),
            goto_text: String::new(),
            search_text: String::new(),
            info_tab: InfoTab::Strings,
        };
        if let Some(path) = std::env::args().nth(1) {
            app.start_load(PathBuf::from(path));
        } else {
            app.status =
                "No binary supplied. Run: tpt-armature-gui <path-to-binary> (or use Open Sample)."
                    .to_string();
        }
        app
    }

    /// Kick off analysis of `path` on a background thread so the UI never
    /// freezes, even on large binaries. Results arrive via `self.load_rx`.
    fn start_load(&mut self, path: PathBuf) {
        let (tx, rx) = channel();
        self.load_rx = Some(rx);
        self.status = format!("loading {} …", path.display());
        self.analysis = None;
        self.cached_cfg = None;
        std::thread::spawn(move || {
            let result = std::fs::read(&path)
                .map_err(|e| format!("cannot read {}: {e}", path.display()))
                .and_then(|bytes| {
                    tpt_armature_analysis::analyze_binary(&bytes)
                        .map_err(|e| format!("analysis failed: {e}"))
                });
            let _ = match result {
                Ok(analysis) => tx.send(LoadMsg::Done {
                    analysis: Box::new(analysis),
                    path,
                }),
                Err(e) => tx.send(LoadMsg::Error(e)),
            };
        });
    }

    /// Poll the background loader; install the result once it is ready.
    fn poll_load(&mut self) {
        if self.load_rx.is_none() {
            return;
        }
        let rx = self.load_rx.take().unwrap();
        match rx.try_recv() {
            Ok(LoadMsg::Done { analysis, path }) => {
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
                let _ = path;
                self.analysis = Some(*analysis);
                self.cached_cfg = None;
            }
            Ok(LoadMsg::Error(e)) => {
                self.status = e;
            }
            Err(TryRecvError::Empty) => {
                // Not ready yet; keep polling next frame.
                self.load_rx = Some(rx);
            }
            Err(TryRecvError::Disconnected) => {
                self.status = "analysis loader disconnected".to_string();
            }
        }
    }
}

impl eframe::App for ArmatureApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_load();

        egui::TopBottomPanel::top("status").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(&self.status);
                if ui.button("Open Sample").clicked() {
                    match sample_path() {
                        Some(p) => self.start_load(p),
                        None => {
                            self.status = "Sample binary not found; run `just build-samples` first."
                                .to_string()
                        }
                    }
                }
                ui.separator();
                ui.label("goto:");
                let goto_r = ui.text_edit_singleline(&mut self.goto_text);
                if goto_r.lost_focus() && ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                    let target = self.goto_text.trim().to_string();
                    self.goto(&target);
                }
                ui.label("search:");
                let search_r = ui.text_edit_singleline(&mut self.search_text);
                if search_r.lost_focus() && ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                    self.search_next(false);
                }
                let editing = goto_r.has_focus() || search_r.has_focus();

                // Keyboard navigation of the Assembly view (only when no text
                // field is focused, so we don't steal cursor keys from the
                // goto/search boxes).
                if !editing {
                    let mut nav = false;
                    if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                        self.step_selected(1);
                        nav = true;
                    }
                    if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                        self.step_selected(-1);
                        nav = true;
                    }
                    if nav {
                        ctx.request_repaint();
                    }
                }
            });
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
                        &mut self.cached_cfg,
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

        self.render_info_panel(ctx);

        #[cfg(feature = "scripts")]
        self.render_script_console(ctx);
    }
}

impl ArmatureApp {
    /// Jump the Assembly view to the instruction at (or first one after) `text`,
    /// which is a hex (`0x…`) or decimal address.
    fn goto(&mut self, text: &str) {
        let Some(analysis) = self.analysis.as_ref() else {
            return;
        };
        let Some(addr) = parse_addr_gui(text) else {
            return;
        };
        if let Some(&idx) = self.addr_to_idx.get(&addr) {
            self.selected = idx;
            self.pending_scroll = Some(idx);
        } else if let Some(pos) = analysis.instructions.iter().position(|i| i.address >= addr) {
            self.selected = pos;
            self.pending_scroll = Some(pos);
        }
    }

    /// Move the Assembly selection by `delta` instructions (clamped).
    fn step_selected(&mut self, delta: i32) {
        let Some(analysis) = self.analysis.as_ref() else {
            return;
        };
        let n = analysis.instructions.len();
        if n == 0 {
            return;
        }
        let cur = self.selected as i32;
        let next = (cur + delta).clamp(0, (n as i32) - 1);
        if next != cur {
            self.selected = next as usize;
            self.pending_scroll = Some(next as usize);
        }
    }

    /// Jump to the next instruction whose text matches [`Self::search_text`]
    /// (wrapping around the end of the listing).
    fn search_next(&mut self, _wrap: bool) {
        let Some(analysis) = self.analysis.as_ref() else {
            return;
        };
        let q = self.search_text.trim().to_ascii_lowercase();
        if q.is_empty() {
            return;
        }
        let n = analysis.instructions.len();
        if n == 0 {
            return;
        }
        for k in 1..=n {
            let idx = (self.selected + k) % n;
            if analysis.instructions[idx]
                .text
                .to_ascii_lowercase()
                .contains(&q)
            {
                self.selected = idx;
                self.pending_scroll = Some(idx);
                return;
            }
        }
    }

    /// Right-hand panel: extracted strings and the pseudocode (decompiler) view.
    fn render_info_panel(&mut self, ctx: &egui::Context) {
        let mut jump_to: Option<u64> = None;
        egui::SidePanel::right("info_panel")
            .resizable(true)
            .default_width(340.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.info_tab, InfoTab::Strings, "Strings");
                    ui.selectable_value(&mut self.info_tab, InfoTab::Pseudocode, "Pseudocode");
                });
                ui.separator();
                let analysis = match self.analysis.as_ref() {
                    Some(a) => a,
                    None => return,
                };
                match self.info_tab {
                    InfoTab::Strings => render_strings(ui, analysis, &mut jump_to),
                    InfoTab::Pseudocode => render_pseudocode(ui, analysis, self.selected_function),
                }
            });
        if let Some(addr) = jump_to {
            let target = format!("0x{addr:x}");
            self.goto_text = target.clone();
            self.goto(&target);
        }
    }
}

/// Locate the QA sample binary (`examples/tpt-armature-sample`) relative to the running
/// executable, walking up to the workspace root.
fn sample_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let mut dir = exe.parent()?;
    let candidates = [
        "examples/tpt-armature-sample/target/release/tpt-armature-sample.exe",
        "examples/tpt-armature-sample/target/release/tpt-armature-sample",
        "examples/tpt-armature-sample/target/debug/tpt-armature-sample.exe",
        "examples/tpt-armature-sample/target/debug/tpt-armature-sample",
    ];
    loop {
        for c in &candidates {
            let p = dir.join(c);
            if p.exists() {
                return Some(p);
            }
        }
        dir = dir.parent()?;
    }
}

#[cfg(feature = "scripts")]
impl ArmatureApp {
    /// Bottom-docked Rhai script console: edit a script, run it against the
    /// loaded analysis, and see the renames it produced (which are also applied
    /// to the function labels in the Graph view).
    fn render_script_console(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("script_console")
            .resizable(true)
            .default_height(160.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Script");
                    if ui.button("Run").clicked() {
                        self.run_script();
                    }
                    if ui.button("Reset").clicked() {
                        self.script_source = default_rename_script().to_string();
                        self.script_output.clear();
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut self.script_source)
                            .code_editor()
                            .desired_width(f32::INFINITY)
                            .desired_rows(6),
                    );
                });
                if !self.script_output.is_empty() {
                    ui.separator();
                    ui.label(&self.script_output);
                }
            });
    }

    /// Execute the current `script_source` against the loaded analysis. Renames
    /// are recorded to `script_output` and applied to the matching functions so
    /// they surface in the Graph view's node labels.
    #[cfg(feature = "scripts")]
    fn run_script(&mut self) {
        let Some(analysis) = self.analysis.as_ref() else {
            self.script_output = "no binary loaded".to_string();
            return;
        };
        let host = ScriptHost::new(analysis);
        match host.run(&self.script_source) {
            Ok(renames) => {
                if let Some(analysis) = self.analysis.as_mut() {
                    for func in analysis.module.functions.iter_mut() {
                        if let Some(name) = renames.get(&func.start) {
                            func.name = Some(name.clone());
                        }
                    }
                }
                if renames.is_empty() {
                    self.script_output = "script produced no renames".to_string();
                } else {
                    let mut entries: Vec<_> = renames.iter().collect();
                    entries.sort_by_key(|(addr, _)| **addr);
                    let mut out = String::from("renames:\n");
                    for (addr, name) in entries {
                        out.push_str(&format!("  0x{addr:x} -> {name}\n"));
                    }
                    self.script_output = out;
                }
            }
            Err(e) => {
                self.script_output = format!("script error: {e}");
            }
        }
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
        if matches!(
            ins.mnemonic,
            Mnemonic::Call | Mnemonic::Jmp | Mnemonic::Jcc(_)
        ) {
            if let Some(target) = ins.operands.iter().find_map(|o| {
                if let Operand::Imm(v) = o {
                    Some(*v)
                } else {
                    None
                }
            }) {
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
    cached_cfg: &mut Option<(usize, Cfg)>,
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
            let label = f.name.clone().unwrap_or_else(|| format!("0x{:x}", f.start));
            ui.selectable_value(selected_function, i, label);
        }
    });

    let idx = (*selected_function).min(funcs.len() - 1);
    let func = &funcs[idx];

    // Cache the per-function CFG: only rebuild when the selection changes (not
    // every frame).
    let rebuild = !matches!(cached_cfg, Some((cached_idx, _)) if *cached_idx == idx);
    if rebuild {
        let single = Module {
            functions: vec![func.clone()],
        };
        let cfg = tpt_armature_analysis::build_cfg(&single);
        *cached_cfg = Some((idx, cfg));
    }
    let cfg = &cached_cfg.as_ref().unwrap().1;

    ui.label(format!(
        "{} blocks, {} edges (function {}/{})",
        cfg.nodes.len(),
        cfg.edges.len(),
        idx + 1,
        funcs.len()
    ));

    let selected_addr = analysis.instructions.get(*selected).map(|i| i.address);
    draw_cfg_canvas(
        ui,
        cfg,
        addr_to_idx,
        selected_addr,
        selected,
        pending_scroll,
    );
}

/// Draw one function's CFG as a layered node-and-edge canvas (not the raw edge
/// list). Nodes are placed in BFS-ranked columns; edges carry arrowheads;
/// clicking a node jumps the Assembly view to that block. The graph is
/// scrollable and capped at [`GRAPH_NODE_CAP`] blocks.
fn draw_cfg_canvas(
    ui: &mut egui::Ui,
    cfg: &tpt_armature_analysis::Cfg,
    addr_to_idx: &HashMap<u64, usize>,
    selected_addr: Option<u64>,
    selected: &mut usize,
    pending_scroll: &mut Option<usize>,
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

            let resp = ui.interact(
                nrect,
                egui::Id::new(("graph_node", i)),
                egui::Sense::click(),
            );
            if resp.clicked() {
                if let Some(&idx) = addr_to_idx.get(&cfg.nodes[i].start) {
                    *selected = idx;
                    *pending_scroll = Some(idx);
                }
            }
        }
    });

    if n > capped {
        ui.label(format!("showing first {capped} of {n} blocks"));
    }
}

/// Draw a small arrowhead at `to` pointing along the `from`->`to` direction.
fn draw_arrowhead(painter: &egui::Painter, from: egui::Pos2, to: egui::Pos2, stroke: egui::Stroke) {
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

/// Strings panel: list the printable strings (ASCII / UTF-16) discovered in the
/// image. Clicking a string jumps the Assembly view to the nearest instruction
/// at or after that address (data references typically resolve to a load nearby).
fn render_strings(ui: &mut egui::Ui, analysis: &Analysis, jump_to: &mut Option<u64>) {
    ui.label(format!("{} string(s)", analysis.strings.len()));
    egui::ScrollArea::vertical().show(ui, |ui| {
        for s in &analysis.strings {
            let kind = match s.kind {
                tpt_armature_analysis::StringKind::Ascii => "ascii",
                tpt_armature_analysis::StringKind::Utf16 => "utf16",
            };
            let label = format!("0x{:08x} [{:5}] {}", s.addr, kind, s.text);
            if ui.selectable_label(false, label).clicked() {
                *jump_to = Some(s.addr);
            }
        }
    });
}

/// Pseudocode panel: render the selected function (mirrors the Graph view's
/// selection) as a C-like listing via the IR decompiler.
fn render_pseudocode(ui: &mut egui::Ui, analysis: &Analysis, selected_function: usize) {
    let funcs = &analysis.module.functions;
    if funcs.is_empty() {
        ui.label("(no functions recovered)");
        return;
    }
    let idx = selected_function.min(funcs.len() - 1);
    let names: std::collections::HashMap<u64, String> = funcs
        .iter()
        .map(|f| {
            (
                f.start,
                f.name
                    .clone()
                    .unwrap_or_else(|| format!("fn_{:x}", f.start)),
            )
        })
        .collect();
    let pseudo = tpt_armature_analysis::decompile_function(&funcs[idx], &names);
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.monospace(pseudo);
    });
}

/// Parse a hex (`0x…`) or decimal address from user input.
fn parse_addr_gui(s: &str) -> Option<u64> {
    let t = s.trim();
    if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        u64::from_str_radix(h, 16).ok()
    } else {
        t.parse::<u64>().ok()
    }
}
