use egui_extras::syntax_highlighting::{highlight, CodeTheme};
use egui_inspect::{
    egui::{self, text::LayoutJob, vec2, ScrollArea},
    logging::{log, setup_mixed_logger, FileLogOption, LogsView},
    utils::type_name_base,
    EframeMain, EguiInspect, InspectNumber,
};
use parser::{parse_rewrite_rules, Expression, RewriteRules};

mod generator;
mod parser;

struct CodeEdit {
    code: String,
    style: egui::Style,
    theme: CodeTheme,
}

static DEFAULT_GRAMMAR: &str = include_str!("../grammar.bnf");

impl Default for CodeEdit {
    fn default() -> Self {
        Self {
            code: DEFAULT_GRAMMAR.to_string(),
            style: Default::default(),
            theme: Default::default(),
        }
    }
}

impl EguiInspect for CodeEdit {
    fn inspect_mut(&mut self, _label: &str, ui: &mut egui::Ui) {
        let mut layouter = |ui: &egui::Ui, string: &str, wrap_width: f32| {
            // TODO: not much different from multiline without highlighting...
            let mut layout_job = highlight(ui.ctx(), &self.style, &self.theme, string, "");
            layout_job.wrap.max_width = wrap_width;
            ui.fonts(|f| f.layout_job(layout_job))
        };

        egui::ScrollArea::vertical()
            .id_salt("code edit")
            .show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut self.code)
                        .font(egui::TextStyle::Monospace) // for cursor height
                        .code_editor()
                        .desired_rows(10)
                        .lock_focus(true)
                        .desired_width(f32::INFINITY)
                        .min_size(vec2(300.0, 200.0))
                        .layouter(&mut layouter),
                );
            });
    }
}

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

#[derive(EframeMain)]
struct ShaderGen {
    code: CodeEdit,
    rr: RewriteRules,
    max_depth: usize,
    generated_r: GeneratedFunc,
    generated_g: GeneratedFunc,
    generated_b: GeneratedFunc,
    feedback: LogsView,
}

impl Default for ShaderGen {
    fn default() -> Self {
        setup_mixed_logger(FileLogOption::DefaultTempDir {
            log_name: format!("{}_log", type_name_base::<Self>()),
        });
        let code = CodeEdit::default();
        let grammar_bytes = code.code.as_bytes();
        let rr = parse_rewrite_rules(grammar_bytes).unwrap();
        Self {
            code,
            rr,
            max_depth: 10,
            generated_r: Default::default(),
            generated_g: Default::default(),
            generated_b: Default::default(),
            feedback: Default::default(),
        }
    }
}

impl EguiInspect for ShaderGen {
    fn inspect_mut(&mut self, _: &str, ui: &mut egui::Ui) {
        ui.columns(2, |cols| {
            let ui = &mut cols[0];
            self.feedback.inspect("Feedback:", ui);
            self.code.inspect_mut("", ui);
            if ui.button("parse grammar").clicked() {
                let grammar_bytes = self.code.code.as_bytes();
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
            ui.horizontal(|ui| {
                self.max_depth
                    .inspect_with_slider("max_depth", ui, 5.0, 25.0);
                if ui.button("generate").clicked() {
                    self.generated_r.regen(&self.rr, self.max_depth);
                    self.generated_g.regen(&self.rr, self.max_depth);
                    self.generated_b.regen(&self.rr, self.max_depth);
                }
            });
            self.generated_r.inspect("r", ui);
            self.generated_g.inspect("g", ui);
            self.generated_b.inspect("b", ui);
        });
    }
}
