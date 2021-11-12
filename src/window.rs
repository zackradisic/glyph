use std::{
    cell::Cell,
    ffi::{c_void, CString},
    mem,
    ptr::null,
};

use gl::types::{GLfloat, GLint, GLsizeiptr, GLuint, GLvoid};
use ropey::RopeSlice;
use sdl2::{
    event::Event,
    keyboard::{Keycode, Mod},
    mouse::MouseWheelDirection,
};

use crate::{
    atlas::Atlas, Editor, EditorEventResult, EventResult, GLProgram, Shader, SCREEN_HEIGHT,
    SCREEN_WIDTH,
};

#[repr(C)]
struct Point {
    x: f32,
    y: f32,
    s: f32,
    t: f32,
}

const SX: f32 = 0.5 / SCREEN_WIDTH as f32;
const SY: f32 = 0.5 / SCREEN_HEIGHT as f32;

const FG: [f32; 4] = [0.92156863, 0.85882354, 0.69803923, 1.0];

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
    cursor_shader: CursorShaderProgram,
    editor: Editor,
    text_coords: Vec<Point>,
    cursor_coords: Vec<f32>,
    y_offset: f32,
    x_offset: f32,
    text_height: f32,
    text_width: f32,
    // Time since last stroke in ms
    last_stroke: u32,
}

impl Window {
    pub fn new() -> Window {
        let font_path = "./fonts/FiraCode.ttf";
        let ft_lib = freetype::Library::init().unwrap();
        let mut face = ft_lib.new_face(font_path, 0).unwrap();

        let text_shader = TextShaderProgram::default();
        let atlas = Atlas::new(&mut face, 48, text_shader.uniform_tex).unwrap();

        let cursor_shader = CursorShaderProgram::default();

        Self {
            atlas,
            text_shader,
            cursor_shader,
            editor: Editor::new(),
            text_coords: Vec::new(),
            cursor_coords: Vec::new(),
            y_offset: 0.0,
            x_offset: 0.0,
            text_height: 0.0,
            text_width: 0.0,
            last_stroke: 0,
        }
    }

    pub fn event(&mut self, event: Event, time: u32) -> EventResult {
        match event {
            Event::Quit { .. } => EventResult::Quit,
            Event::KeyDown {
                keycode: Some(Keycode::C),
                keymod,
                ..
            } if keymod == Mod::LCTRLMOD => EventResult::Quit,
            Event::MouseWheel { x, y, .. } => {
                if x.abs() > y.abs() {
                    self.scroll_x(x as f32 * -4.0);
                } else {
                    self.scroll_y(y as f32 * 4.0);
                }
                self.render_text();
                EventResult::Draw
            }
            _ => match self.editor.event(event) {
                EditorEventResult::DrawText => {
                    self.last_stroke = time;
                    self.render_text();
                    EventResult::Draw
                }
                EditorEventResult::DrawCursor => {
                    self.queue_cursor();
                    EventResult::Draw
                }
                _ => EventResult::Nothing,
            },
        }
    }
}

// This impl contains utilities
impl Window {
    fn scroll_y(&mut self, amount: f32) {
        match amount > 0.0 {
            true => {
                if self.y_offset + amount >= 0.0 {
                    self.y_offset = 0.0;
                } else {
                    self.y_offset += amount;
                }
            }
            false => {
                if -1.0 * (self.y_offset + amount) >= self.text_height {
                    self.y_offset = self.text_height * -1.0;
                } else {
                    self.y_offset += amount;
                }
            }
        }
    }

    fn scroll_x(&mut self, amount: f32) {
        match amount > 0.0 {
            true => {
                if self.x_offset + amount >= 0.0 {
                    self.x_offset = 0.0;
                } else {
                    self.x_offset += amount;
                }
            }
            false => {
                if -1.0 * (self.x_offset + amount) >= self.text_width {
                    self.x_offset = self.text_width * -1.0;
                } else {
                    self.x_offset += amount;
                }
            }
        }
    }
}

// This impl contains graphics functions
impl Window {
    pub fn queue_cursor(&mut self) {
        let w = self.atlas.max_w * SX;
        let real_h = self.atlas.max_h * SY;
        let h = (self.atlas.max_h/*+ 5f32*/) * SY;

        let x = (-1f32 + 8f32 * SX)
            + (self.editor.cursor() as f32 * (w/*+ self.atlas.glyphs[35].advance_x * SX*/));
        let y = ((1f32 - 50f32 * SY) + real_h) - (self.editor.line() as f32 * real_h);

        self.cursor_coords = vec![
            // bottom left
            x,
            y - h,
            0.0,
            // top left
            x,
            y,
            0.0,
            // top right
            x + w,
            y,
            0.0,
            // bottom right
            x + w,
            y - h,
            0.0,
            // top right,
            x + w,
            y,
            0.0,
            // bottom leff
            x,
            y - h,
            0.0,
        ];
    }

    fn render_text(&mut self) {
        self.queue_cursor();
        self.queue_text(-1f32 + 8f32 * SX, 1f32 - 50f32 * SY, SX, SY);
    }

    pub fn frame(&self, ticks_ms: u32) {
        self.text_shader.set_used();

        // Draw text
        unsafe {
            gl::Uniform4fv(self.text_shader.uniform_color, 1, FG.as_ptr() as *const f32);
            gl::VertexAttrib1f(self.text_shader.attrib_ytranslate, SY * self.y_offset);
            gl::VertexAttrib1f(self.text_shader.attrib_xtranslate, self.x_offset * SX);

            // Use the texture containing the atlas
            gl::BindTexture(gl::TEXTURE_2D, self.atlas.tex);
            gl::Uniform1i(self.text_shader.uniform_tex, 0);

            // Set up the VBO for our vertex data
            gl::EnableVertexAttribArray(self.text_shader.attrib_coord);
            gl::BindBuffer(gl::ARRAY_BUFFER, self.text_shader.vbo);

            gl::VertexAttribPointer(
                self.text_shader.attrib_coord,
                4,
                gl::FLOAT,
                gl::FALSE,
                0,
                null(),
            );

            gl::BufferData(
                gl::ARRAY_BUFFER,
                (self.text_coords.len() * mem::size_of::<Point>()) as GLsizeiptr,
                self.text_coords.as_ptr() as *const GLvoid,
                gl::DYNAMIC_DRAW,
            );
            gl::DrawArrays(gl::TRIANGLES, 0, self.text_coords.len() as i32);
            gl::DisableVertexAttribArray(self.text_shader.attrib_coord);
        }

        // Draw cursor
        self.cursor_shader.set_used();
        unsafe {
            gl::VertexAttrib1f(self.cursor_shader.attrib_ytranslate, self.y_offset * SY);
            gl::VertexAttrib1f(self.cursor_shader.attrib_xtranslate, self.x_offset * SX);
            gl::Uniform1f(
                self.cursor_shader.uniform_laststroke,
                self.last_stroke as f32 / 1000.0,
            );
            gl::Uniform1i(
                self.cursor_shader.uniform_is_blinking,
                if self.editor.is_insert() { 1 } else { 0 },
            );
            gl::Uniform1f(self.cursor_shader.uniform_time, ticks_ms as f32 / 1000.0);
        }

        let mut vbo: GLuint = 0;
        let attrib_ptr = self.cursor_shader.attrib_apos;
        unsafe {
            gl::GenBuffers(1, &mut vbo);
            gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
            gl::BlendFunci(1, gl::SRC_ALPHA, gl::ONE);
            gl::BlendEquationi(1, gl::FUNC_SUBTRACT);
            gl::BufferData(
                gl::ARRAY_BUFFER,
                (18 * mem::size_of::<f32>()).try_into().unwrap(),
                self.cursor_coords.as_ptr() as *const c_void,
                gl::DYNAMIC_DRAW,
            );

            gl::VertexAttribPointer(
                attrib_ptr,
                3,
                gl::FLOAT,
                gl::FALSE,
                3 * mem::size_of::<f32>() as i32,
                null(),
            );
            gl::EnableVertexAttribArray(0);
            gl::DrawArrays(gl::TRIANGLES, 0, 6);
            gl::DisableVertexAttribArray(0);
        }
    }

    fn queue_text(&mut self, mut x: f32, mut y: f32, sx: f32, sy: f32) {
        let text = self.editor.text_all();
        let starting_x = x;
        // TODO: Cache this
        let mut coords: Vec<Point> = Vec::with_capacity(6 * text.len_chars());

        let mut text_height = 0.0;
        let mut line_width = 0.0;

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

            line_width += self.atlas.glyphs[c].advance_x;

            // Skip glyphs that have no pixels
            if width == 0.0 || height == 0.0 {
                if ch == '\n' {
                    y -= self.atlas.max_h * sy;
                    text_height += self.atlas.max_h;
                    self.text_height = self.text_height.max(line_width);
                    line_width = 0.0;
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

        // TODO: It's probably faster to directly mutate the vec instead of making a
        // new one and replacing it
        self.text_coords = coords;
        self.text_height = text_height;
        self.text_width = self.text_width.max(line_width);
    }
}

impl Default for Window {
    fn default() -> Self {
        Self::new()
    }
}

pub struct TextShaderProgram {
    program: GLProgram,
    attrib_coord: GLuint,
    attrib_ytranslate: GLuint,
    attrib_xtranslate: GLuint,
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
            attrib_coord: program.attrib("coord").unwrap() as u32,
            attrib_ytranslate: program.attrib("y_translate").unwrap() as u32,
            attrib_xtranslate: program.attrib("x_translate").unwrap() as u32,
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

pub struct CursorShaderProgram {
    program: GLProgram,
    attrib_ytranslate: GLuint,
    attrib_xtranslate: GLuint,
    uniform_time: GLint,
    uniform_laststroke: GLint,
    uniform_is_blinking: GLint,
    attrib_apos: GLuint,
}

impl CursorShaderProgram {
    pub fn new() -> Self {
        let shaders = vec![
            Shader::from_source(
                &CString::new(include_str!("../shaders/cursor.v.glsl")).unwrap(),
                gl::VERTEX_SHADER,
            )
            .unwrap(),
            Shader::from_source(
                &CString::new(include_str!("../shaders/cursor.f.glsl")).unwrap(),
                gl::FRAGMENT_SHADER,
            )
            .unwrap(),
        ];

        let program = GLProgram::from_shaders(&shaders).unwrap();

        Self {
            attrib_apos: program.attrib("aPos").unwrap() as u32,
            attrib_ytranslate: program.attrib("y_translate").unwrap() as u32,
            attrib_xtranslate: program.attrib("x_translate").unwrap() as u32,
            uniform_time: program.uniform("time").unwrap(),
            uniform_laststroke: program.uniform("last_stroke").unwrap(),
            uniform_is_blinking: program.uniform("is_blinking").unwrap(),
            program,
        }
    }

    #[inline]
    pub fn set_used(&self) {
        self.program.set_used()
    }
}

impl Default for CursorShaderProgram {
    fn default() -> Self {
        Self::new()
    }
}
