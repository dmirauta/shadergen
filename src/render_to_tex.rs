use egui_inspect::{
    eframe::{
        self,
        glow::{
            self, HasContext, NativeFramebuffer, NativeProgram, NativeTexture, NativeVertexArray,
            PixelUnpackData,
        },
    },
    egui::TextureId,
    logging::log::warn,
};
use std::sync::Arc;

pub struct TexFramebuffer {
    pub id: NativeFramebuffer,
    pub tex: CustomTex,
}

#[macro_export]
macro_rules! print_text_on_err {
    ($gl: ident) => {{
        use egui_inspect::logging::log::error;
        match $gl.get_error() {
            0 => {}
            glow::INVALID_ENUM => error!("Invalid enum"),
            glow::INVALID_VALUE => error!("Invalid value"),
            glow::INVALID_OPERATION => error!("Invalid operation"),
            e => error!("Error code {e}"),
        }
    }};
}

/// don't mess up the window framebuffer... otherwise nothing gets drawn
fn with_save_restore_outer_fb<F>(gl: &Arc<glow::Context>, render_operations: F)
where
    F: Fn(&Arc<glow::Context>),
{
    unsafe {
        let outerfb = gl.get_parameter_framebuffer(glow::DRAW_FRAMEBUFFER_BINDING);
        print_text_on_err!(gl);
        render_operations(gl);
        print_text_on_err!(gl);
        gl.bind_framebuffer(glow::FRAMEBUFFER, outerfb);
        // TODO: do we need to restore the viewport also? (seemingly not)
    }
}

impl TexFramebuffer {
    pub fn new(gl: &Arc<glow::Context>, width: usize, height: usize) -> Self {
        unsafe {
            let fb = gl.create_framebuffer().expect("cannot create framebuffer");

            let mut ct = CustomTex::new(gl, false);
            print_text_on_err!(gl);

            ct.update_rgb(gl, None, width, height);
            print_text_on_err!(gl);

            with_save_restore_outer_fb(gl, |gl| {
                gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fb));
                print_text_on_err!(gl);
                gl.framebuffer_texture(
                    glow::FRAMEBUFFER,
                    glow::COLOR_ATTACHMENT0,
                    Some(ct.nativetex),
                    0,
                );
            });
            print_text_on_err!(gl);

            Self { id: fb, tex: ct }
        }
    }
    /// render to this texture
    pub fn render_here<F>(&self, gl: &Arc<glow::Context>, render_operations: F)
    where
        F: Fn(&Arc<glow::Context>),
    {
        unsafe {
            with_save_restore_outer_fb(gl, |gl| {
                gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.id));
                print_text_on_err!(gl);
                gl.viewport(0, 0, self.tex.width as i32, self.tex.height as i32);
                print_text_on_err!(gl);
                render_operations(gl);
            });
            print_text_on_err!(gl);
        }
    }
}

pub struct CustomTex {
    nativetex: NativeTexture,
    eguitex: Option<TextureId>,
    width: usize,
    height: usize,
}

impl CustomTex {
    pub fn new(gl: &Arc<glow::Context>, wrap: bool) -> Self {
        unsafe {
            let nativetex = gl.create_texture().expect("cannot create texture");
            gl.bind_texture(glow::TEXTURE_2D, Some(nativetex));

            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                glow::LINEAR as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                glow::LINEAR as i32,
            );

            if wrap {
                gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::REPEAT as i32);
                gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::REPEAT as i32);
            }

            Self {
                nativetex,
                eguitex: None,
                width: 0,
                height: 0,
            }
        }
    }
    pub fn egui_tid(&mut self, frame: &mut eframe::Frame) -> TextureId {
        *self
            .eguitex
            .get_or_insert_with(|| frame.register_native_glow_texture(self.nativetex))
    }
    fn _update(
        &mut self,
        gl: &Arc<glow::Context>,
        data_raw: Option<&[u8]>,
        width: usize,
        height: usize,
        fmt: u32,
    ) {
        self.width = width;
        self.height = height;
        unsafe {
            gl.bind_texture(glow::TEXTURE_2D, Some(self.nativetex));
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                fmt as i32,
                width as i32,
                height as i32,
                0,
                fmt,
                glow::UNSIGNED_BYTE,
                PixelUnpackData::Slice(data_raw),
            );
        }
    }
    pub fn update_rgb(
        &mut self,
        gl: &Arc<glow::Context>,
        data_raw: Option<&[u8]>,
        width: usize,
        height: usize,
    ) {
        self._update(gl, data_raw, width, height, glow::RGB);
    }
    #[allow(dead_code)]
    pub fn update_rgba(
        &mut self,
        gl: &Arc<glow::Context>,
        data_raw: Option<&[u8]>,
        width: usize,
        height: usize,
    ) {
        self._update(gl, data_raw, width, height, glow::RGBA);
    }
}

/// a quad which covers the whole screenspace
pub struct ScreenspaceQuad {
    pub id: NativeVertexArray,
    pub prog: Option<NativeProgram>,
}

static LARGE_TRI_VERT_SHADER: &str = r#"
const vec2 verts[3] = vec2[3](
    vec2(-1.0f, 1.0f),  // bottom left
    vec2(3.0f, 1.0f),   // twice as far as bottom right (from bottom left)
    vec2(-1.0f, -3.0f)  // twice as far as top left (from bottom left)
);
out vec2 uv;
void main() {
    vec2 vert = verts[gl_VertexID];
    uv = (vert+1.0)/2.0; // creates [0,1]x[0,1] range in the visible portion (whole tex)
    gl_Position = vec4(vert, 0.0, 1.0);
}"#;
#[allow(dead_code)]
pub static TEST_FRAG_SHADER: &str = r#"
precision mediump float;
in vec2 uv;
out vec4 color;
void main() {
    color = vec4(uv, 0.0, 1.0);
}"#;

impl ScreenspaceQuad {
    pub fn new(gl: &Arc<glow::Context>, fragment_shader_source: &str) -> Self {
        unsafe {
            let vertex_array = gl
                .create_vertex_array()
                .expect("Cannot create vertex array");

            let mut new = Self {
                id: vertex_array,
                prog: None,
            };
            if let Err(e) = new.set_frag_shader(gl, fragment_shader_source) {
                warn!("Could not set frag shader: {e}")
            }
            new
        }
    }
    // NOTE: adapted from https://github.com/grovesNL/glow/blob/main/examples/hello/src/main.rs
    pub fn set_frag_shader(
        &mut self,
        gl: &Arc<glow::Context>,
        fragment_shader_source: &str,
    ) -> Result<(), String> {
        unsafe {
            gl.bind_vertex_array(Some(self.id));

            let program = gl.create_program().expect("Cannot create program");

            let mut shaders = Vec::with_capacity(2);

            // HACK: Somewhat cheating by drawing a triangle that's sufficiently bigger than the
            // screenspace and still mapping the fragment shader correctly after scaling and
            // clipping
            // NOTE: version may need changing for web
            let shader_version = "#version 410";
            for (shader_type, shader_source) in [
                (glow::VERTEX_SHADER, LARGE_TRI_VERT_SHADER),
                (glow::FRAGMENT_SHADER, fragment_shader_source),
            ] {
                let shader = gl.create_shader(shader_type).expect("Cannot create shader");
                gl.shader_source(shader, &format!("{}\n{}", shader_version, shader_source));
                gl.compile_shader(shader);
                if !gl.get_shader_compile_status(shader) {
                    return Err(gl.get_shader_info_log(shader));
                }
                gl.attach_shader(program, shader);
                shaders.push(shader);
            }

            gl.link_program(program);
            if !gl.get_program_link_status(program) {
                return Err(gl.get_program_info_log(program));
            }

            for shader in shaders {
                gl.detach_shader(program, shader);
                gl.delete_shader(shader);
            }

            gl.use_program(Some(program));

            if let Some(old_prog) = self.prog {
                gl.delete_program(old_prog);
            }
            self.prog = Some(program);
        }
        Ok(())
    }
    pub fn set_f32_uniform(&self, gl: &Arc<glow::Context>, name: &str, value: f32) {
        unsafe {
            gl.use_program(self.prog);
            if let Some(prog) = self.prog {
                let loc = gl.get_uniform_location(prog, name); // NOTE: we could save this...
                print_text_on_err!(gl);
                gl.uniform_1_f32(loc.as_ref(), value);
                print_text_on_err!(gl);
            } else {
                warn!("program not set");
            }
        }
    }
}
