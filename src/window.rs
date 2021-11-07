use std::{ffi::CString, mem, ptr::null};

use gl::types::{GLint, GLsizeiptr, GLuint, GLvoid};
use ropey::RopeSlice;
use sdl2::{
    event::Event,
    keyboard::{Keycode, Mod},
    video::GLContext,
    video::Window as SdlWindow,
    Sdl, VideoSubsystem,
};

use crate::{atlas::Atlas, Editor, EventResult, GLProgram, Shader, SCREEN_HEIGHT, SCREEN_WIDTH};

#[repr(C)]
struct Point {
    x: f32,
    y: f32,
    s: f32,
    t: f32,
}

const SX: f32 = 0.5 / SCREEN_WIDTH as f32;
const SY: f32 = 0.5 / SCREEN_HEIGHT as f32;

fn hex_to_rgba(hex: &str) -> [f32; 4] {
    let mut rgba = [0.0, 0.0, 0.0, 1.0];
    let hex = hex.trim_start_matches('#');
    for (i, c) in hex
        .chars()
        .step_by(2)
        .zip(hex.chars().skip(1).step_by(2))
        .enumerate()
    {
        let c = c.0.to_digit(16).unwrap() << 4 | c.1.to_digit(16).unwrap();
        rgba[i] = c as f32 / 255.0;
    }
    rgba
}

pub struct Window {
    atlas: Atlas,
    text_shader: TextShaderProgram,
    editor: Editor,
}

impl Window {
    pub fn new() -> Window {
        let font_path = "./fonts/FiraCode.ttf";
        let ft_lib = freetype::Library::init().unwrap();
        let mut face = ft_lib.new_face(font_path, 0).unwrap();

        let text_shader = TextShaderProgram::default();
        let atlas = Atlas::new(&mut face, 48, text_shader.uniform_tex).unwrap();

        text_shader.set_used();

        Self {
            atlas,
            text_shader,
            editor: Editor::new(),
        }
    }

    pub fn event(&mut self, event: Event) -> EventResult {
        match event {
            Event::Quit { .. } => EventResult::Quit,
            Event::KeyDown {
                keycode: Some(Keycode::C),
                keymod,
                ..
            } if keymod == Mod::LCTRLMOD => EventResult::Quit,
            _ => match self.editor.event(event) {
                EventResult::Draw => {
                    let slice = self.editor.text_all();
                    self.render_text(slice);
                    EventResult::Draw
                }
                r => r,
            },
        }
    }

    fn render_text(&self, text: RopeSlice) {
        let fg = hex_to_rgba("ebdbb2");
        unsafe {
            gl::ClearColor(0.157, 0.157, 0.157, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
            gl::Uniform4fv(self.text_shader.uniform_color, 1, fg.as_ptr() as *const f32);
        }
        self.queue_text(text, -1f32 + 8f32 * SX, 1f32 - 50f32 * SY, SX, SY);
    }

    fn queue_text(&self, text: RopeSlice, mut x: f32, mut y: f32, sx: f32, sy: f32) {
        let starting_x = x;
        unsafe {
            // Use the texture containing the atlas
            gl::BindTexture(gl::TEXTURE_2D, self.atlas.tex);
            gl::Uniform1i(self.text_shader.uniform_tex, 0);

            // Set up the VBO for our vertex data
            gl::EnableVertexAttribArray(self.text_shader.attribute_coord);
            gl::BindBuffer(gl::ARRAY_BUFFER, self.text_shader.vbo);
            gl::VertexAttribPointer(
                self.text_shader.attribute_coord,
                4,
                gl::FLOAT,
                gl::FALSE,
                0,
                null(),
            );
        }

        // TODO: Cache this
        let mut coords: Vec<Point> = Vec::with_capacity(6 * text.len_chars());
        // let mut coords: Vec<Point> = Vec::with_capacity(6 * text.len());

        for ch in text.chars() {
            let c = ch as usize;

            // Calculate the vertex and texture coordinates
            let x2 = x + self.atlas.glyphs[c].bitmap_l * sx;
            let y2 = -y - self.atlas.glyphs[c].bitmap_t * sy;
            let width = self.atlas.glyphs[c].bitmap_w * sx;
            let height = self.atlas.glyphs[c].bitmap_h * sy;

            // Advance the cursor to the start of the next character
            x += self.atlas.glyphs[c].advance_x * sx;
            y += self.atlas.glyphs[c].advance_y * sy;

            // Skip glyphs that have no pixels
            if width == 0.0 || height == 0.0 {
                if ch == '\n' {
                    y -= self.atlas.max_h * sy;
                    x = starting_x;
                }
                continue;
            }

            coords.push(Point {
                x: x2,
                y: -y2,
                s: self.atlas.glyphs[c].tx,
                t: self.atlas.glyphs[c].ty,
            });

            coords.push(Point {
                x: x2 + width,
                y: -y2,
                s: self.atlas.glyphs[c].tx + self.atlas.glyphs[c].bitmap_w / self.atlas.w as f32,
                t: self.atlas.glyphs[c].ty,
            });

            coords.push(Point {
                x: x2,
                y: -y2 - height,
                s: self.atlas.glyphs[c].tx,
                t: self.atlas.glyphs[c].ty + self.atlas.glyphs[c].bitmap_h / self.atlas.h as f32,
            });

            coords.push(Point {
                x: x2 + width,
                y: -y2,
                s: self.atlas.glyphs[c].tx + self.atlas.glyphs[c].bitmap_w / self.atlas.w as f32,
                t: self.atlas.glyphs[c].ty,
            });

            coords.push(Point {
                x: x2,
                y: -y2 - height,
                s: self.atlas.glyphs[c].tx,
                t: self.atlas.glyphs[c].ty + self.atlas.glyphs[c].bitmap_h / self.atlas.h as f32,
            });

            coords.push(Point {
                x: x2 + width,
                y: -y2 - height,
                s: self.atlas.glyphs[c].tx + self.atlas.glyphs[c].bitmap_w / self.atlas.w as f32,
                t: self.atlas.glyphs[c].ty + self.atlas.glyphs[c].bitmap_h / self.atlas.h as f32,
            });
        }

        unsafe {
            gl::BufferData(
                gl::ARRAY_BUFFER,
                (coords.len() * mem::size_of::<Point>()) as GLsizeiptr,
                coords.as_ptr() as *const GLvoid,
                gl::DYNAMIC_DRAW,
            );
            gl::DrawArrays(gl::TRIANGLES, 0, coords.len() as i32);

            gl::DisableVertexAttribArray(self.text_shader.attribute_coord);
        }
    }
}

impl Default for Window {
    fn default() -> Self {
        Self::new()
    }
}

pub struct TextShaderProgram {
    program: GLProgram,
    attribute_coord: GLuint,
    uniform_tex: GLint,
    uniform_color: GLint,
    vbo: GLuint,
}

impl TextShaderProgram {
    pub fn new() -> Self {
        let shaders = vec![
            Shader::from_source(
                &CString::new(include_str!("../shaders/text.v.glsl")).unwrap(),
                gl::VERTEX_SHADER,
            )
            .unwrap(),
            Shader::from_source(
                &CString::new(include_str!("../shaders/text.f.glsl")).unwrap(),
                gl::FRAGMENT_SHADER,
            )
            .unwrap(),
        ];

        let program = GLProgram::from_shaders(&shaders).unwrap();

        let mut vbo: GLuint = 0;
        unsafe {
            gl::GenBuffers(1, &mut vbo as *mut GLuint);
        }

        Self {
            attribute_coord: program.attrib("coord").unwrap() as u32,
            uniform_tex: program.uniform("tex").unwrap(),
            uniform_color: program.uniform("color").unwrap(),
            vbo,
            program,
        }
    }

    #[inline]
    pub fn set_used(&self) {
        self.program.set_used()
    }
}

impl Default for TextShaderProgram {
    fn default() -> Self {
        Self::new()
    }
}
