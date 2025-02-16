use egui_extras::syntax_highlighting::{highlight, CodeTheme};
use egui_inspect::{
    egui::{self, text::LayoutJob, vec2},
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

        egui::ScrollArea::vertical().show(ui, |ui| {
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

#[derive(EframeMain)]
struct ShaderGen {
    code: CodeEdit,
    rr: RewriteRules,
    max_depth: usize,
    generated: Box<Expression>,
    generated_str: String,
}

impl Default for ShaderGen {
    fn default() -> Self {
        let code = CodeEdit::default();
        let grammar_bytes = code.code.as_bytes();
        let rr = parse_rewrite_rules(grammar_bytes).unwrap();
        Self {
            code,
            rr,
            max_depth: 10,
            generated: Box::new(Expression::ToBeReplaced {
                rule: "C".to_string(),
            }),
            generated_str: Default::default(),
        }
    }
}

impl EguiInspect for ShaderGen {
    fn inspect_mut(&mut self, _: &str, ui: &mut egui::Ui) {
        ui.columns(2, |cols| {
            let ui = &mut cols[0];
            self.code.inspect_mut("", ui);
            if ui.button("parse grammar").clicked() {
                let grammar_bytes = self.code.code.as_bytes();
                match parse_rewrite_rules(grammar_bytes) {
                    Ok(rr) => {
                        self.rr = rr;
                    }
                    Err(e) => {
                        // TODO: log feedback
                        println!("parse error: {e:?}");
                    }
                }
            }
            ui.horizontal(|ui| {
                self.max_depth
                    .inspect_with_slider("max_depth", ui, 5.0, 25.0);
                if ui.button("generate").clicked() {
                    self.generated = self.rr.gen_fn(self.max_depth);
                    self.generated_str = self.generated.as_string();
                }
            });
            if !self.generated_str.is_empty() {
                let mut job = LayoutJob::default();
                job.wrap.max_width = ui.available_width();
                job.append(&self.generated_str, 0.0, Default::default());
                ui.label("generated func:");
                ui.label(job);
                // TODO: repeat for each colour channel
            }
        });
    }
}
