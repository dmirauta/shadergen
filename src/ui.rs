use egui_extras::syntax_highlighting::{highlight, CodeTheme};
use egui_inspect::{
    egui::{self, vec2},
    EguiInspect,
};

pub struct CodeEdit {
    pub code: String,
    style: egui::Style,
    theme: CodeTheme,
    lang: String,
    pub height: f32,
}

impl CodeEdit {
    pub fn new(code: String, lang: String) -> Self {
        Self {
            code,
            lang,
            style: Default::default(),
            theme: Default::default(),
            height: 150.0,
        }
    }
}

impl EguiInspect for CodeEdit {
    fn inspect_mut(&mut self, label: &str, ui: &mut egui::Ui) {
        let mut layouter = |ui: &egui::Ui, string: &str, wrap_width: f32| {
            let mut layout_job = highlight(
                ui.ctx(),
                &self.style,
                &self.theme,
                string,
                self.lang.as_str(),
            );
            layout_job.wrap.max_width = wrap_width;
            ui.fonts(|f| f.layout_job(layout_job))
        };

        egui::ScrollArea::vertical()
            .id_salt(label)
            .max_height(self.height)
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
