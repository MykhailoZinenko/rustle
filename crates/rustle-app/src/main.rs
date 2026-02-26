use eframe::egui::{self, Color32, RichText};
use rustle_lang::{
    syntax::lexer::Lexer,
    syntax::parser::Parser,
    analysis::{self, symbols::SymbolKind},
    namespaces::NamespaceRegistry,
    compile, Runtime, DrawCommand, Input, Origin, RenderMode, ShapeData, ShapeDesc,
};
use rustle_lang::analysis::checker::type_name;


/// Return screen pixel vertices (0,0 = top-left, y-down).
fn tessellate_screen_px(data: &ShapeData) -> Vec<(f64, f64)> {
    let m = &data.coord_meta;
    let sx = |x: f64| m.x_to_screen_px(x);
    let sy = |y: f64| m.y_to_screen_px(y);

    let verts = match &data.desc {
        ShapeDesc::Circle { center, radius } => {
            (0..64usize).map(|i| {
                let t = i as f64 / 64.0 * std::f64::consts::TAU;
                (sx(center.0 + radius * t.cos()), sy(center.1 + radius * t.sin()))
            }).collect()
        }
        ShapeDesc::Rect { center, size, origin } => {
            let (w, h) = (size.0, size.1);
            let (ax, ay) = (sx(center.0), sy(center.1));
            let (min_x, max_x) = match origin {
                Origin::TopLeft | Origin::BottomLeft | Origin::Left
                    => (ax, ax + w),
                Origin::TopRight | Origin::BottomRight | Origin::Right
                    => (ax - w, ax),
                Origin::Center | Origin::Top | Origin::Bottom
                    => (ax - w / 2.0, ax + w / 2.0),
            };
            let (min_y, max_y) = match origin {
                Origin::TopLeft | Origin::TopRight | Origin::Top
                    => (ay, ay + h),
                Origin::BottomLeft | Origin::BottomRight | Origin::Bottom
                    => (ay - h, ay),
                Origin::Center | Origin::Left | Origin::Right
                    => (ay - h / 2.0, ay + h / 2.0),
            };
            vec![
                (min_x, min_y), (max_x, min_y),
                (max_x, max_y), (min_x, max_y),
            ]
        }
        ShapeDesc::Line { from, to } => vec![
            (sx(from.0), sy(from.1)),
            (sx(to.0),   sy(to.1)),
        ],
        ShapeDesc::Polygon(pts) => pts.iter()
            .map(|(x, y)| (sx(*x), sy(*y)))
            .collect(),
    };

    // Apply transforms in screen pixel space.
    // td.tx / td.ty are in user-space units — convert to screen px deltas:
    //   x-right origins flip the x direction, y-up origins flip the y direction.
    let x_sign: f64 = match m.origin {
        Origin::TopRight | Origin::BottomRight | Origin::Right => -1.0,
        _ => 1.0,
    };
    let y_sign: f64 = if m.origin.is_y_down() { 1.0 } else { -1.0 };

    let mut result = verts;
    for td in &data.transforms {
        let tx_px = td.tx * x_sign;
        let ty_px = td.ty * y_sign;
        let n = result.len() as f64;
        let (sum_x, sum_y) = result.iter().fold((0.0, 0.0), |(ax, ay), (x, y)| (ax + x, ay + y));
        let (pivot_x, pivot_y) = (sum_x / n, sum_y / n);
        // Rotation angle: td.angle is CCW in math (y-up) space.
        // In screen pixels (y-down) the y axis is flipped, so CCW math = CW visually.
        // To keep the same visual rotation as NDC, negate the angle.
        let a = -td.angle;
        let (cos_a, sin_a) = (a.cos(), a.sin());
        result = result.into_iter().map(|(x, y)| {
            let dx = (x - pivot_x) * td.sx;
            let dy = (y - pivot_y) * td.sy;
            let rx = dx * cos_a - dy * sin_a;
            let ry = dx * sin_a + dy * cos_a;
            (pivot_x + rx + tx_px, pivot_y + ry + ty_px)
        }).collect();
    }
    result
}

/// Convert a ShapeData to NDC vertices for display in the output tab.
fn tessellate(data: &ShapeData) -> Vec<(f64, f64)> {
    let m = &data.coord_meta;
    let verts = match &data.desc {
        ShapeDesc::Circle { center, radius } => {
            let cx = m.x_to_ndc(center.0);
            let cy = m.y_to_ndc(center.1);
            let r  = m.w_to_ndc(*radius);
            (0..64usize).map(|i| {
                let t = i as f64 / 64.0 * std::f64::consts::TAU;
                (cx + r * t.cos(), cy + r * t.sin())
            }).collect()
        }
        ShapeDesc::Rect { center, size, origin } => {
            let (w, h) = (size.0, size.1);
            let ax = m.x_to_screen_px(center.0);
            let ay = m.y_to_screen_px(center.1);
            let (min_x, max_x) = match origin {
                Origin::TopLeft | Origin::BottomLeft | Origin::Left
                    => (ax, ax + w),
                Origin::TopRight | Origin::BottomRight | Origin::Right
                    => (ax - w, ax),
                Origin::Center | Origin::Top | Origin::Bottom
                    => (ax - w / 2.0, ax + w / 2.0),
            };
            let (min_y, max_y) = match origin {
                Origin::TopLeft | Origin::TopRight | Origin::Top
                    => (ay, ay + h),
                Origin::BottomLeft | Origin::BottomRight | Origin::Bottom
                    => (ay - h, ay),
                Origin::Center | Origin::Left | Origin::Right
                    => (ay - h / 2.0, ay + h / 2.0),
            };
            let snx = |s: f64| if m.px_width  > 0.0 { 2.0 * s / m.px_width  - 1.0 } else { s };
            let sny = |s: f64| if m.px_height > 0.0 { 1.0 - 2.0 * s / m.px_height } else { s };
            vec![
                (snx(min_x), sny(min_y)), (snx(max_x), sny(min_y)),
                (snx(max_x), sny(max_y)), (snx(min_x), sny(max_y)),
            ]
        }
        ShapeDesc::Line { from, to } => vec![
            (m.x_to_ndc(from.0), m.y_to_ndc(from.1)),
            (m.x_to_ndc(to.0),   m.y_to_ndc(to.1)),
        ],
        ShapeDesc::Polygon(pts) => pts.iter()
            .map(|(x, y)| (m.x_to_ndc(*x), m.y_to_ndc(*y)))
            .collect(),
    };

    // Apply accumulated transforms in NDC space
    let mut result = verts;
    for td in &data.transforms {
        let tx = m.w_to_ndc(td.tx);
        let ty = m.dy_to_ndc(td.ty);
        let (cos_a, sin_a) = (td.angle.cos(), td.angle.sin());
        let n = result.len() as f64;
        let (sum_x, sum_y) = result.iter().fold((0.0, 0.0), |(ax, ay), (x, y)| (ax + x, ay + y));
        let (pivot_x, pivot_y) = (sum_x / n, sum_y / n);
        result = result.into_iter().map(|(x, y)| {
            let dx = (x - pivot_x) * td.sx;
            let dy = (y - pivot_y) * td.sy;
            let rx = dx * cos_a - dy * sin_a;
            let ry = dx * sin_a + dy * cos_a;
            (pivot_x + rx + tx, pivot_y + ry + ty)
        }).collect();
    }
    result
}

fn fmt_origin(o: &Origin) -> &'static str {
    match o {
        Origin::Center      => "center",
        Origin::TopLeft     => "top_left",
        Origin::TopRight    => "top_right",
        Origin::BottomLeft  => "bottom_left",
        Origin::BottomRight => "bottom_right",
        Origin::Top         => "top",
        Origin::Bottom      => "bottom",
        Origin::Left        => "left",
        Origin::Right       => "right",
    }
}

fn mono_row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).monospace().color(Color32::from_rgb(140, 140, 140)));
        ui.label(RichText::new(value).monospace().color(Color32::from_rgb(210, 210, 170)));
    });
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1400.0, 800.0]),
        ..Default::default()
    };
    eframe::run_native("Rustle Dev", options, Box::new(|_cc| Ok(Box::new(App::default()))))
}

// ─── App state ────────────────────────────────────────────────────────────────

#[derive(PartialEq)]
enum Tab { Errors, Symbols, Ast, Output, Canvas }

struct App {
    source: String,
    result: RunResult,
    tab: Tab,
    show_builtins: bool,
    runtime: Option<Runtime>,
    last_tick: std::time::Instant,
}

impl Default for App {
    fn default() -> Self {
        let source = String::from(
"// write rustle code here
import shapes { circle, rect }

let s = circle(vec2(0.5, 0.5), 0.2)
out << s
");
        let result = run(&source, false);
        let runtime = compile(&source).ok().and_then(|p| Runtime::new(p).ok());
        Self { source, result, tab: Tab::Canvas, show_builtins: false, runtime, last_tick: std::time::Instant::now() }
    }
}

// ─── Run result ───────────────────────────────────────────────────────────────

struct SymbolRow {
    name: String,
    ty: String,
    kind: String,
    is_builtin: bool,
}

struct RunResult {
    errors: Vec<String>,
    symbols: Vec<SymbolRow>,
    ast: String,
    draw_commands: Vec<DrawCommand>,
}

fn run(source: &str, _show_builtins: bool) -> RunResult {
    let mut errors: Vec<String> = Vec::new();

    // ── Lex ───────────────────────────────────────────────────────────────────
    let tokens = match Lexer::new(source).tokenize() {
        Ok(t) => t,
        Err(errs) => {
            return RunResult {
                errors: errs.iter().map(|e| format!("[lex] {e}")).collect(),
                symbols: vec![],
                ast: String::new(),
                draw_commands: vec![],
            };
        }
    };

    // ── Parse ─────────────────────────────────────────────────────────────────
    let program = match Parser::new(tokens).parse() {
        Ok(p) => p,
        Err(errs) => {
            return RunResult {
                errors: errs.iter().map(|e| format!("[parse] {e}")).collect(),
                symbols: vec![],
                ast: String::new(),
                draw_commands: vec![],
            };
        }
    };

    let ast = format!("{program:#?}");

    // ── Resolve ───────────────────────────────────────────────────────────────
    let registry = NamespaceRegistry::standard();
    let (symbols, resolve_errors) = match analysis::resolve(&program, &registry) {
        Ok(result) => {
            errors.extend(result.warnings.iter().map(|e| format!("[warn] {e}")));
            (result.symbol_table, vec![])
        }
        Err(errs) => {
            let msgs = errs.iter().map(|e| format!("[semantic] {e}")).collect();
            // Still try to show partial symbol table by re-running collector
            use rustle_lang::analysis::collector::Collector;
            let (partial_table, _) = Collector::new(&registry).collect(&program);
            (partial_table, msgs)
        }
    };

    errors.extend(resolve_errors);

    // ── Build symbol rows ─────────────────────────────────────────────────────
    let symbol_rows = symbols.global_symbols().into_iter()
        .filter(|s| !s.name.starts_with("__state__") || s.kind == SymbolKind::StateField)
        .map(|s| {
            let is_builtin = s.span.line == 0; // core symbols have span (0,0)
            let display_name = if s.name.starts_with("__state__") {
                s.name.strip_prefix("__state__").unwrap_or(&s.name).to_string()
            } else {
                s.name.clone()
            };
            SymbolRow {
                name: display_name,
                ty: s.ty.as_ref().map(type_name).unwrap_or_else(|| "?".into()),
                kind: match s.kind {
                    SymbolKind::Function   => "fn",
                    SymbolKind::Const      => "const",
                    SymbolKind::Variable   => "let",
                    SymbolKind::Param      => "param",
                    SymbolKind::StateField => "state",
                }.into(),
                is_builtin,
            }
        })
        .collect();

    RunResult { errors, symbols: symbol_rows, ast, draw_commands: vec![] }
}

// ─── UI ───────────────────────────────────────────────────────────────────────

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // ── Tick runtime every frame ──────────────────────────────────────────
        let now = std::time::Instant::now();
        let dt = now.duration_since(self.last_tick).as_secs_f64().min(0.1);
        self.last_tick = now;

        if let Some(rt) = &mut self.runtime {
            let input = Input { dt };
            match rt.tick(&input) {
                Ok(cmds) => self.result.draw_commands = cmds,
                Err(e) => {
                    self.result.errors.push(format!("[runtime] {}", e.message));
                    self.runtime = None;
                }
            }
            ctx.request_repaint();
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.columns(2, |cols| {
                // ── Left: editor ──────────────────────────────────────────────
                cols[0].vertical(|ui| {
                    ui.label("Source");
                    let response = ui.add(
                        egui::TextEdit::multiline(&mut self.source)
                            .font(egui::TextStyle::Monospace)
                            .desired_width(f32::INFINITY)
                            .desired_rows(44),
                    );
                    if response.changed() {
                        self.result = run(&self.source, self.show_builtins);
                        self.runtime = compile(&self.source).ok().and_then(|p| Runtime::new(p).ok());
                        self.last_tick = std::time::Instant::now();
                    }
                });

                // ── Right: output ─────────────────────────────────────────────
                cols[1].vertical(|ui| {
                    // ── Status bar ────────────────────────────────────────────
                    ui.horizontal(|ui| {
                        let error_count = self.result.errors.iter()
                            .filter(|e| !e.starts_with("[warn]"))
                            .count();
                        if error_count == 0 {
                            ui.label(RichText::new("✓  no errors").color(Color32::from_rgb(80, 200, 80)));
                        } else {
                            ui.label(RichText::new(format!("✗  {error_count} error(s)")).color(Color32::from_rgb(220, 80, 80)));
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.checkbox(&mut self.show_builtins, "show builtins");
                            if ui.button("run").clicked() {
                                self.result = run(&self.source, self.show_builtins);
                                self.runtime = compile(&self.source).ok().and_then(|p| Runtime::new(p).ok());
                                self.last_tick = std::time::Instant::now();
                            }
                        });
                    });

                    ui.separator();

                    // ── Tab bar ───────────────────────────────────────────────
                    ui.horizontal(|ui| {
                        let err_label = if self.result.errors.is_empty() {
                            "Errors".into()
                        } else {
                            format!("Errors ({})", self.result.errors.len())
                        };
                        ui.selectable_value(&mut self.tab, Tab::Errors, err_label);
                        ui.selectable_value(&mut self.tab, Tab::Symbols, "Symbols");
                        ui.selectable_value(&mut self.tab, Tab::Ast, "AST");
                        ui.selectable_value(&mut self.tab, Tab::Output, "Output");
                        ui.selectable_value(&mut self.tab, Tab::Canvas, "Canvas");
                    });

                    ui.separator();

                    // ── Tab content ───────────────────────────────────────────
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        match self.tab {
                            Tab::Errors  => self.show_errors(ui),
                            Tab::Symbols => self.show_symbols(ui),
                            Tab::Ast     => self.show_ast(ui),
                            Tab::Output  => self.show_output(ui),
                            Tab::Canvas  => self.show_canvas(ui),
                        }
                    });
                });
            });
        });
    }
}

impl App {
    fn show_output(&self, ui: &mut egui::Ui) {
        if self.result.draw_commands.is_empty() {
            let msg = if self.result.errors.iter().any(|e| !e.starts_with("[warn]")) {
                "Fix errors to run."
            } else {
                "No draw commands — add out << shape"
            };
            ui.label(RichText::new(msg).color(Color32::GRAY));
            return;
        }

        for (i, cmd) in self.result.draw_commands.iter().enumerate() {
            let DrawCommand::DrawShape(data) = cmd;

            let mode_str = match &data.render_mode {
                RenderMode::Sdf       => "sdf".to_string(),
                RenderMode::Fill      => "fill".to_string(),
                RenderMode::Outline   => "outline".to_string(),
                RenderMode::Stroke(w) => format!("stroke({w:.3})"),
            };

            let shape_name = match &data.desc {
                ShapeDesc::Circle { .. } => "circle",
                ShapeDesc::Rect { .. }   => "rect",
                ShapeDesc::Line { .. }   => "line",
                ShapeDesc::Polygon(_)    => "polygon",
            };

            // ── Header ───────────────────────────────────────────────────────
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("[{}]", i + 1)).monospace().color(Color32::GRAY));
                ui.label(RichText::new("DrawShape").strong());
                ui.label(RichText::new(shape_name).monospace().color(Color32::from_rgb(180, 140, 255)));
                ui.label(RichText::new(&mode_str).monospace().color(Color32::from_rgb(120, 180, 255)));
            });

            // ── Raw struct ───────────────────────────────────────────────────
            let v2 = |x: f64, y: f64| format!("({:.3}, {:.3})", x, y);

            match &data.desc {
                ShapeDesc::Circle { center, radius } => {
                    mono_row(ui, "  center:", &v2(center.0, center.1));
                    mono_row(ui, "  radius:", &format!("{:.3}", radius));
                }
                ShapeDesc::Rect { center, size, origin } => {
                    mono_row(ui, "  center:", &v2(center.0, center.1));
                    mono_row(ui, "  size:  ", &v2(size.0, size.1));
                    mono_row(ui, "  origin:", fmt_origin(origin));
                }
                ShapeDesc::Line { from, to } => {
                    mono_row(ui, "  from:", &v2(from.0, from.1));
                    mono_row(ui, "  to:  ", &v2(to.0, to.1));
                }
                ShapeDesc::Polygon(pts) => {
                    let pts_str: Vec<String> = pts.iter().map(|(x, y)| v2(*x, *y)).collect();
                    mono_row(ui, "  pts:", &pts_str.join(", "));
                }
            }

            // ── CoordMeta ────────────────────────────────────────────────────
            let m = &data.coord_meta;
            mono_row(ui, "  meta:", &format!(
                "{}×{}px  origin={}",
                m.px_width as u32, m.px_height as u32,
                fmt_origin(&m.origin)
            ));

            // ── Transforms ───────────────────────────────────────────────────
            for (ti, td) in data.transforms.iter().enumerate() {
                mono_row(ui, &format!("  tf[{}]:", ti), &format!(
                    "move=({:.3}, {:.3})  scale=({:.3}, {:.3})  rot={:.1}°",
                    td.tx, td.ty, td.sx, td.sy, td.angle.to_degrees()
                ));
            }

            // ── Vertex previews ───────────────────────────────────────────────
            let fmt_verts = |verts: &[(f64, f64)]| -> String {
                let preview: Vec<String> = verts.iter().take(4)
                    .map(|(x, y)| format!("({x:.3}, {y:.3})"))
                    .collect();
                let ellipsis = if verts.len() > 4 { ", …" } else { "" };
                format!("{}{}", preview.join(", "), ellipsis)
            };
            let raw_verts = tessellate_screen_px(data);
            mono_row(ui, "  px: ", &fmt_verts(&raw_verts));
            let ndc_verts = tessellate(data);
            mono_row(ui, "  ndc:", &fmt_verts(&ndc_verts));

            ui.add_space(8.0);
        }
    }

    fn show_canvas(&self, ui: &mut egui::Ui) {
        if self.result.draw_commands.is_empty() {
            let msg = if self.result.errors.iter().any(|e| !e.starts_with("[warn]")) {
                "Fix errors to run."
            } else {
                "No draw commands — add out << shape"
            };
            ui.label(RichText::new(msg).color(Color32::GRAY));
            return;
        }

        // Determine canvas size from first command's coord_meta, or default 400×400.
        let (canvas_w, canvas_h) = {
            let first = match &self.result.draw_commands[0] { DrawCommand::DrawShape(d) => d };
            let m = &first.coord_meta;
            if m.px_width > 0.0 && m.px_height > 0.0 {
                (m.px_width as f32, m.px_height as f32)
            } else {
                (400.0_f32, 400.0_f32)
            }
        };

        let desired = egui::vec2(canvas_w, canvas_h);
        let (canvas_rect, _response) = ui.allocate_exact_size(desired, egui::Sense::hover());
        let painter = ui.painter_at(canvas_rect);

        // Background
        painter.rect_filled(canvas_rect, 0.0, Color32::from_rgb(28, 28, 32));

        // Draw each shape
        for cmd in &self.result.draw_commands {
            let DrawCommand::DrawShape(data) = cmd;

            let screen_verts = tessellate_screen_px(data);
            if screen_verts.is_empty() { continue; }

            // Offset vertices by canvas top-left so they map onto the allocated rect.
            let offset = canvas_rect.min;
            let pts: Vec<egui::Pos2> = screen_verts.iter()
                .map(|(x, y)| egui::pos2(offset.x + *x as f32, offset.y + *y as f32))
                .collect();

            let fill_color   = Color32::from_rgba_unmultiplied(180, 160, 255, 200);
            let stroke_color = Color32::from_rgba_unmultiplied(200, 180, 255, 255);
            let stroke_width = match &data.render_mode {
                RenderMode::Stroke(w) => *w as f32,
                _ => 1.5_f32,
            };
            let stroke = egui::Stroke::new(stroke_width, stroke_color);

            let is_line = matches!(&data.desc, ShapeDesc::Line { .. });

            if is_line {
                if pts.len() >= 2 {
                    painter.line_segment([pts[0], pts[1]], stroke);
                }
            } else {
                match &data.render_mode {
                    RenderMode::Fill | RenderMode::Sdf => {
                        painter.add(egui::Shape::convex_polygon(
                            pts,
                            fill_color,
                            egui::Stroke::NONE,
                        ));
                    }
                    RenderMode::Outline | RenderMode::Stroke(_) => {
                        painter.add(egui::Shape::closed_line(pts, stroke));
                    }
                }
            }
        }
    }

    fn show_errors(&self, ui: &mut egui::Ui) {
        if self.result.errors.is_empty() {
            ui.label(RichText::new("No errors.").color(Color32::GRAY));
            return;
        }
        for msg in &self.result.errors {
            let color = if msg.starts_with("[warn]") {
                Color32::from_rgb(220, 180, 60)
            } else {
                Color32::from_rgb(220, 80, 80)
            };
            ui.label(RichText::new(msg).monospace().color(color));
        }
    }

    fn show_symbols(&self, ui: &mut egui::Ui) {
        let visible: Vec<&SymbolRow> = self.result.symbols.iter()
            .filter(|s| self.show_builtins || !s.is_builtin)
            .collect();

        if visible.is_empty() {
            ui.label(RichText::new("No symbols.").color(Color32::GRAY));
            return;
        }

        egui::Grid::new("symbols_grid")
            .striped(true)
            .min_col_width(80.0)
            .show(ui, |ui| {
                // Header
                ui.label(RichText::new("name").strong());
                ui.label(RichText::new("kind").strong());
                ui.label(RichText::new("type").strong());
                ui.end_row();

                for row in &visible {
                    ui.label(RichText::new(&row.name).monospace());

                    let kind_color = match row.kind.as_str() {
                        "fn"    => Color32::from_rgb(100, 180, 255),
                        "const" => Color32::from_rgb(255, 200, 80),
                        "let"   => Color32::from_rgb(180, 255, 180),
                        _       => Color32::GRAY,
                    };
                    ui.label(RichText::new(&row.kind).monospace().color(kind_color));
                    ui.label(RichText::new(&row.ty).monospace().color(Color32::from_rgb(200, 200, 200)));
                    ui.end_row();
                }
            });
    }

    fn show_ast(&self, ui: &mut egui::Ui) {
        if self.result.ast.is_empty() {
            ui.label(RichText::new("No AST (parse failed).").color(Color32::GRAY));
            return;
        }
        ui.add(
            egui::TextEdit::multiline(&mut self.result.ast.clone())
                .font(egui::TextStyle::Monospace)
                .desired_width(f32::INFINITY)
                .interactive(false),
        );
    }
}
