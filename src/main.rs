use egui_inspect::{
    eframe::{self, CreationContext},
    egui::{self, text::LayoutJob, CentralPanel, ScrollArea},
    logging::{
        log::{self, error, info, warn},
        setup_mixed_logger, FileLogOption, LogsView,
    },
    utils::type_name_base,
    EframeMain, EguiInspect, InspectNumber,
};
use parser::{parse_rewrite_rules, Expression, RewriteRules};
use ui::{CodeEdit, EguiFragShaderPreview};

mod funcgen;
mod parser;
mod render_to_tex;
mod ui;

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
    // TODO: not much different from multiline without highlighting...
    grammar: CodeEdit,
    frag: CodeEdit,
    rr: RewriteRules,
    max_depth: usize,
    generated_r: GeneratedFunc,
    generated_g: GeneratedFunc,
    generated_b: GeneratedFunc,
    feedback: LogsView,
    gl_viewport: EguiFragShaderPreview,
}

static DEFAULT_GRAMMAR: &str = include_str!("../grammar.bnf");
static DEFAULT_FRAG: &str = include_str!("../default_frag.glsl");

impl ShaderGen {
    fn init(cc: &CreationContext) -> Self {
        setup_mixed_logger(FileLogOption::DefaultTempDir {
            log_name: format!("{}_log", type_name_base::<Self>()),
        });
        let gcode = DEFAULT_GRAMMAR.to_string();
        let grammar = CodeEdit::new(gcode, "".to_string());
        let fcode = DEFAULT_FRAG.to_string();
        let frag = CodeEdit::new(fcode, "c".to_string()); // not c, but it will have to do...
        let grammar_bytes = DEFAULT_GRAMMAR.as_bytes();
        let rr = parse_rewrite_rules(grammar_bytes).unwrap();
        Self {
            grammar,
            frag,
            rr,
            max_depth: 10,
            generated_r: Default::default(),
            generated_g: Default::default(),
            generated_b: Default::default(),
            feedback: Default::default(),
            gl_viewport: EguiFragShaderPreview::init(cc, DEFAULT_FRAG),
        }
    }
    fn _insert_channel_funcs(&mut self) -> Option<()> {
        let rdec = find_var_decl_line(&self.frag.code, "r")?;
        let gdec = find_var_decl_line(&self.frag.code, "g")?;
        let bdec = find_var_decl_line(&self.frag.code, "b")?;
        let mut lines: Vec<_> = self.frag.code.lines().map(|l| l.to_string()).collect();
        lines[rdec] = format!("    float r = {};", &self.generated_r.generated_str);
        lines[gdec] = format!("    float g = {};", &self.generated_g.generated_str);
        lines[bdec] = format!("    float b = {};", &self.generated_b.generated_str);
        self.frag.code = lines.join("\n");
        Some(())
    }
    fn generate_funcs(&mut self) {
        self.generated_r.regen(&self.rr, self.max_depth);
        self.generated_g.regen(&self.rr, self.max_depth);
        self.generated_b.regen(&self.rr, self.max_depth);
    }
    fn insert_channel_funcs(&mut self) {
        if self._insert_channel_funcs().is_none() {
            warn!("{}", RGB_DECL_WARN);
        }
    }
    fn compile_shader(&mut self, frame: &mut eframe::Frame) {
        if let Some(gl) = frame.gl() {
            if let Err(e) = self.gl_viewport.quad.set_frag_shader(gl, &self.frag.code) {
                error!("Failed to compile frag shader: {e}");
            }
        }
    }
}

static RGB_DECL_WARN: &str = "Inserting functions into shader failed, please keep the formatting of the r,g,b declarations similar to the default shader (no additional spacing between tokens, each kept on one line, not declared twice, even in other functions).";

impl eframe::App for ShaderGen {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        CentralPanel::default().show(ctx, |ui| {
            ui.columns(2, |cols| {
                let ui = &mut cols[0];
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
                        self.compile_shader(frame);
                    }
                });

                let ui = &mut cols[1];
                self.gl_viewport._update(ui, frame);

                ui.horizontal(|ui| {
                    if ui.button("generate, insert and compile").clicked() {
                        self.generate_funcs();
                        self.insert_channel_funcs();
                        self.compile_shader(frame);
                    }
                })
                .response
                .on_hover_text(
                    "Apply the actions on the left all in one step, without editing intermediates.",
                );
            });
        });
    }
}
