use crate::{
    print_text_on_err,
    render_to_tex::{ScreenspaceQuad, TexFramebuffer},
};
use egui_extras::syntax_highlighting::{highlight, CodeTheme};
use egui_inspect::{
    eframe::{
        self,
        glow::{self, HasContext},
        CreationContext,
    },
    egui::{self, vec2},
    EguiInspect,
};
use std::{
    sync::Arc,
    time::{Duration, SystemTime},
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

pub struct EguiFragShaderPreview {
    /// an alternative framebuffer, backed by a texture which we will display as an egui image
    fb: TexFramebuffer,
    /// the geometry which will be rastered to the framebuffer (with the shader we supply)
    pub quad: ScreenspaceQuad,
    gl: Arc<glow::Context>,
    t0: SystemTime,
}

impl EguiFragShaderPreview {
    pub fn init(cc: &CreationContext, init_shader: &str) -> Self {
        let gl = cc.gl.as_ref().unwrap().clone();
        let fb = TexFramebuffer::new(&gl, 1024, 768);
        let quad = ScreenspaceQuad::new(&gl, init_shader);
        let t0 = SystemTime::now();
        let new = Self { fb, quad, gl, t0 };
        new.draw_quad_to_tex();
        new
    }
    pub fn draw_quad_to_tex(&self) {
        self.fb.render_here(&self.gl, |gl| unsafe {
            gl.bind_vertex_array(Some(self.quad.id));
            print_text_on_err!(gl);
            gl.draw_arrays(glow::TRIANGLES, 0, 3);
        });
    }
    pub fn _update(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ft = (self.t0.elapsed().unwrap().as_millis() as f32) / 1000.0;
        self.quad.set_f32_uniform(&self.gl, "t", ft);
        self.draw_quad_to_tex();
        ui.image((self.fb.tex.egui_tid(frame), vec2(800.0, 600.)));
        ui.ctx()
            .request_repaint_after(Duration::from_millis(1000 / 30));
    }
}
