use std::sync::{Arc, Mutex};

use egui_inspect::{
    eframe::{
        self, egui_glow,
        glow::{self, HasContext},
        CreationContext,
    },
    egui::{
        self, text::LayoutJob, vec2, CentralPanel, LayerId, ScrollArea, Sense, Shape, TextEdit,
        Window,
    },
    logging::{
        default_mixed_logger,
        log::{self, error, info, warn},
        LogsView,
    },
    EframeMain, EguiInspect, InspectNumber,
};
use funcgen::{RNG, SRNG};
use parser::{parse_rewrite_rules, Expression, RewriteRules};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use ui::CodeEdit;
use viewport_quad::ViewportQuad;

mod funcgen;
mod parser;
mod ui;
mod viewport_quad;

struct GeneratedFunc {
    generated: Box<Expression>,
    generated_str: String,
    pub height: f32,
}

impl Default for GeneratedFunc {
    fn default() -> Self {
        Self {
            generated: Box::new(Expression::Terminal(parser::Term::T)),
            generated_str: Default::default(),
            height: 50.0,
        }
    }
}

impl GeneratedFunc {
    fn regen(&mut self, rr: &RewriteRules, max_depth: usize) {
        self.generated = rr.gen_fn(max_depth);
        self.generated_str = self.generated.as_string();
    }
}

impl EguiInspect for GeneratedFunc {
    fn inspect(&self, label: &str, ui: &mut egui::Ui) {
        if !self.generated_str.is_empty() {
            ui.label(format!("generated {label} func:"));
            ScrollArea::vertical()
                .id_salt(label)
                .max_height(self.height)
                .show(ui, |ui| {
                    let mut job = LayoutJob::default();
                    job.wrap.max_width = ui.available_width();
                    job.append(&self.generated_str, 0.0, Default::default());
                    ui.label(job);
                });
        }
    }
}

fn find_var_decl_line(code: &str, var: &str) -> Option<usize> {
    // NOTE: will fail if spacing varies around these tokens...
    let prefix = format!("float {var} = ");
    let suffix = ";";
    for (i, line) in code.lines().enumerate() {
        if line.trim().starts_with(prefix.as_str()) {
            if line.trim().ends_with(suffix) {
                return Some(i);
            } else {
                info!("Declaration for variable {var} found, but line does not end in \";\". To keep things simple, please keep the declaration all on one line.");
            }
        }
    }
    None
}

#[derive(EframeMain)]
#[eframe_main(no_eframe_app_derive, init = "ShaderGen::init(_cc)")]
struct ShaderGen {
    grammar: CodeEdit,
    frag: CodeEdit,
    rr: RewriteRules,
    max_depth: usize,
    generated_r: GeneratedFunc,
    generated_g: GeneratedFunc,
    generated_b: GeneratedFunc,
    feedback: LogsView,
    gl: Arc<glow::Context>,
    gl_viewport: Arc<Mutex<ViewportQuad>>,
    next_seed: u64,
    next_seed_str: String,
    last_seed: u64,
    t: f64,
    t_max: f64,
    play: bool,
    advancing: bool,
}

static DEFAULT_GRAMMAR: &str = include_str!("../grammar.bnf");
static DEFAULT_FRAG: &str = include_str!("../default_frag.glsl");
const ASPECT: f32 = 9.0 / 16.0;

impl ShaderGen {
    fn init(cc: &CreationContext) -> Self {
        default_mixed_logger::<Self>();
        let gcode = DEFAULT_GRAMMAR.to_string();
        // TODO: not much different from multiline without highlighting...
        let grammar = CodeEdit::new(gcode, "".to_string());
        let fcode = DEFAULT_FRAG.to_string();
        let frag = CodeEdit::new(fcode, "c".to_string()); // not c, but it will have to do...
        let grammar_bytes = DEFAULT_GRAMMAR.as_bytes();
        let rr = parse_rewrite_rules(grammar_bytes).unwrap();
        let gl = cc.gl.as_ref().unwrap().clone();
        let next_seed = SRNG.write().unwrap().random();
        let mut new = Self {
            grammar,
            frag,
            rr,
            max_depth: 10,
            generated_r: Default::default(),
            generated_g: Default::default(),
            generated_b: Default::default(),
            feedback: Default::default(),
            gl_viewport: Arc::new(Mutex::new(ViewportQuad::new(&gl, DEFAULT_FRAG))),
            gl,
            next_seed_str: format!("{next_seed}"),
            next_seed,
            last_seed: 0,
            t: 0.0,
            t_max: 10.0,
            play: true,
            advancing: true,
        };
        new.generate_funcs();
        new.insert_channel_funcs();
        new.compile_shader();
        new
    }
    fn _insert_channel_funcs(&mut self) -> Option<()> {
        let rdec = find_var_decl_line(&self.frag.code, "red")?;
        let gdec = find_var_decl_line(&self.frag.code, "green")?;
        let bdec = find_var_decl_line(&self.frag.code, "blue")?;
        let mut lines: Vec<_> = self.frag.code.lines().collect();
        let rline = format!("    float red = {};", &self.generated_r.generated_str);
        let gline = format!("    float green = {};", &self.generated_g.generated_str);
        let bline = format!("    float blue = {};", &self.generated_b.generated_str);
        lines[rdec] = rline.as_str();
        lines[gdec] = gline.as_str();
        lines[bdec] = bline.as_str();
        self.frag.code = lines.join("\n");
        Some(())
    }
    fn generate_funcs(&mut self) {
        self.last_seed = self.next_seed;
        *RNG.write().unwrap() = ChaCha8Rng::seed_from_u64(self.next_seed);
        self.generated_r.regen(&self.rr, self.max_depth);
        self.generated_g.regen(&self.rr, self.max_depth);
        self.generated_b.regen(&self.rr, self.max_depth);
        // advance seed for next time
        self.next_seed = SRNG.write().unwrap().random();
        self.next_seed_str = format!("{}", self.next_seed);
    }
    fn insert_channel_funcs(&mut self) {
        if self._insert_channel_funcs().is_none() {
            warn!("{}", RGB_DECL_WARN);
        }
    }
    fn compile_shader(&mut self) {
        if let Err(e) = self
            .gl_viewport
            .lock()
            .unwrap()
            .set_frag_shader(&self.gl, &self.frag.code)
        {
            error!("Failed to compile frag shader: {e}");
        }
    }
    fn paint_viewport(&self, ui: &mut egui::Ui) {
        let available = ui.available_size();
        let size = match available.y / ASPECT < available.x {
            true => vec2(available.y / ASPECT, available.y),
            false => vec2(available.x, available.x * ASPECT),
        };
        let (rect, _) = ui.allocate_exact_size(size, Sense::empty());

        let t = self.t;
        let view = self.gl_viewport.clone();
        ui.ctx()
            .layer_painter(LayerId::background())
            .add(Shape::Callback(egui::PaintCallback {
                rect,
                callback: Arc::new(egui_glow::CallbackFn::new(move |_, painter| {
                    if let Ok(vp) = view.try_lock() {
                        let gl = painter.gl();
                        unsafe {
                            pogle!(gl, gl.use_program(vp.prog));
                            pogle!(gl, gl.bind_vertex_array(Some(vp.va)));

                            if let Some(prog) = vp.prog {
                                let loc = pogle!(gl, gl.get_uniform_location(prog, "t"));
                                pogle!(gl, gl.uniform_1_f32(loc.as_ref(), t as f32));
                            }

                            pogle!(gl, gl.draw_arrays(glow::TRIANGLES, 0, 3));
                        }
                    }
                })),
            }));
    }
}

static RGB_DECL_WARN: &str = "Inserting functions into shader failed, please keep the formatting of the r,g,b declarations similar to the default shader (no additional spacing between tokens, each kept on one line, not declared twice, even in other functions).";

impl eframe::App for ShaderGen {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        CentralPanel::default().show(ctx, |ui| {
            ui.ctx().request_repaint();
            if self.play {
                if self.advancing {
                    self.t += ui.input(|i| i.stable_dt as f64) * self.t_max / 3.0;
                } else {
                    self.t -= ui.input(|i| i.stable_dt as f64) * self.t_max / 3.0;
                }
            }
            if self.t > self.t_max {
                self.t = self.t_max;
                self.advancing = !self.advancing;
            }
            if self.t < 0.0 {
                self.t = 0.0;
                self.advancing = !self.advancing;
            }

            ui.horizontal_wrapped(|ui| {
                if ui.button("generate, insert and compile").clicked() {
                    self.generate_funcs();
                    self.insert_channel_funcs();
                    self.compile_shader();
                }
                self.play.inspect_mut("play", ui);
                self.t.inspect_with_slider("t", ui, 0.0, self.t_max as f32);
                self.t_max.inspect_mut("slider t_max", ui);

                ui.label("next seed:");
                if ui
                    .add(
                        TextEdit::singleline(&mut self.next_seed_str)
                            .char_limit(64)
                            .desired_width(150.0),
                    )
                    .changed()
                {
                    match self.next_seed_str.parse::<u64>() {
                        Ok(v) => self.next_seed = v,
                        Err(_) => self.next_seed_str = format!("{}", self.next_seed),
                    }
                }

                ui.label(format!("last seed: {}", self.last_seed));
            });
            self.paint_viewport(ui);
        });
        Window::new("Grammar and shader generation intermediates").show(ctx, |ui| {
            self.feedback.inspect("Feedback:", ui);
            self.grammar.inspect_mut("Grammar: ", ui);

            // HACK: single buttons have been put in hboxes to stop them taking up full
            // width
            ui.horizontal(|ui| {
                if ui.button("parse grammar").clicked() {
                    let grammar_bytes = self.grammar.code.as_bytes();
                    match parse_rewrite_rules(grammar_bytes) {
                        Ok(rr) => {
                            self.rr = rr;
                            log::info!("Succesfully parsed grammar.");
                        }
                        Err(e) => {
                            // TODO: log feedback
                            log::error!("Parse error: {e:?}");
                        }
                    }
                }
            });
            ui.horizontal(|ui| {
                self.max_depth
                    .inspect_with_slider("max_depth", ui, 5.0, 25.0);
                if ui.button("generate channel functions").clicked() {
                    self.generate_funcs();
                }
            });
            self.generated_r.inspect("r", ui);
            self.generated_g.inspect("g", ui);
            self.generated_b.inspect("b", ui);

            ui.horizontal(|ui| {
                if ui.button("insert funcs into shader code").clicked() {
                    self.insert_channel_funcs();
                }
                if ui.button("reset shader code to default").clicked() {
                    self.frag.code = DEFAULT_FRAG.to_string();
                }
            });

            self.frag.inspect_mut("Fragment: ", ui);

            ui.horizontal(|ui| {
                if ui.button("compile shader").clicked() {
                    self.compile_shader();
                }
            });
        });
    }
}
