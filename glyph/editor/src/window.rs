use std::{
    ffi::{c_void, CString},
    mem,
    ptr::null,
};

use gl::types::{GLint, GLsizeiptr, GLuint, GLvoid};
use once_cell::sync::Lazy;
use sdl2::{
    event::Event,
    keyboard::{Keycode, Mod},
};
use syntax::tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};
use syntax::Highlight;

use crate::{
    atlas::Atlas, Color, Editor, EditorEventResult, EventResult, GLProgram, Shader, ThemeType,
    SCREEN_HEIGHT, SCREEN_WIDTH,
};

#[repr(C)]
struct Point {
    x: f32,
    y: f32,
    s: f32,
    t: f32,
}
const SX: f32 = 1.0 / SCREEN_WIDTH as f32;
const SY: f32 = 1.0 / SCREEN_HEIGHT as f32;

pub struct Window<'theme, 'highlight> {
    // Graphics
    atlas: Atlas,
    text_shader: TextShaderProgram,
    cursor_shader: CursorShaderProgram,
    editor: Editor,
    text_coords: Vec<Point>,
    text_colors: Vec<Color>,
    cursor_coords: [f32; 18],
    y_offset: f32,
    x_offset: f32,
    text_height: f32,
    text_width: f32,

    // Time since last stroke in ms
    last_stroke: u32,
    theme: &'theme ThemeType,

    // Syntax highlighting
    highlighter: Highlighter,
    highlight_cfg: &'highlight Lazy<HighlightConfiguration>,
}

impl<'theme, 'highlight> Window<'theme, 'highlight> {
    pub fn new(initial_text: Option<String>, theme: &'theme ThemeType) -> Self {
        let font_path = "./fonts/FiraCode.ttf";
        let ft_lib = freetype::Library::init().unwrap();
        let mut face = ft_lib.new_face(font_path, 0).unwrap();

        let text_shader = TextShaderProgram::default();
        let atlas = Atlas::new(&mut face, 48, text_shader.uniform_tex).unwrap();

        let cursor_shader = CursorShaderProgram::default();

        let highlighter = Highlighter::new();

        let f = syntax::RUST_CFG.names();
        println!("HIGHLIGHT NAMES: {:#?}", f);

        Self {
            atlas,
            text_shader,
            cursor_shader,
            editor: Editor::with_text(initial_text),
            text_coords: Vec::new(),
            text_colors: Vec::new(),
            cursor_coords: Default::default(),
            y_offset: 0.0,
            x_offset: 0.0,
            text_height: 0.0,
            text_width: 0.0,
            last_stroke: 0,
            theme,
            highlighter,
            highlight_cfg: &syntax::RUST_CFG,
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
                    self.scroll_y(y as f32 * 8.0);
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
impl<'theme, 'highlight> Window<'theme, 'highlight> {
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
impl<'theme, 'highlight> Window<'theme, 'highlight> {
    pub fn render_text(&mut self) {
        self.queue_cursor();
        let colors = self.queue_highlights();
        self.queue_text(colors, -1f32 + 8f32 * SX, 1f32 - 50f32 * SY, SX, SY);
    }

    pub fn queue_cursor(&mut self) {
        let w = self.atlas.max_w * SX;
        let real_h = self.atlas.max_h * SY;
        let h = (self.atlas.max_h/*+ 5f32*/) * SY;

        let x = (-1f32 + 8f32 * SX)
            + (self.editor.cursor() as f32 * (w/*+ self.atlas.glyphs[35].advance_x * SX*/));
        let y = ((1f32 - 50f32 * SY) + real_h) - (self.editor.line() as f32 * real_h);

        self.cursor_coords = [
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

    pub fn frame(&self, ticks_ms: u32) {
        self.text_shader.set_used();

        // Draw text
        unsafe {
            gl::VertexAttrib1f(self.text_shader.attrib_ytranslate, SY * self.y_offset);
            gl::VertexAttrib1f(self.text_shader.attrib_xtranslate, self.x_offset * SX);

            // Use the texture containing the atlas
            gl::BindTexture(gl::TEXTURE_2D, self.atlas.tex);
            gl::Uniform1i(self.text_shader.uniform_tex, 0);

            // Set up the VBO for our vertex data
            gl::BindBuffer(gl::ARRAY_BUFFER, self.text_shader.vbo);
            gl::VertexAttribPointer(
                self.text_shader.attrib_coord,
                4,
                gl::FLOAT,
                gl::FALSE,
                0,
                null(),
            );
            gl::EnableVertexAttribArray(self.text_shader.attrib_coord);
            gl::BufferData(
                gl::ARRAY_BUFFER,
                (self.text_coords.len() * mem::size_of::<Point>()) as GLsizeiptr,
                self.text_coords.as_ptr() as *const GLvoid,
                gl::DYNAMIC_DRAW,
            );

            gl::BindBuffer(gl::ARRAY_BUFFER, self.text_shader.vbo_color);
            gl::VertexAttribPointer(
                self.text_shader.attrib_v_color,
                4,
                gl::FLOAT,
                gl::FALSE,
                0,
                null(),
            );
            gl::EnableVertexAttribArray(self.text_shader.attrib_v_color);
            gl::BufferData(
                gl::ARRAY_BUFFER,
                (self.text_colors.len() * mem::size_of::<Color>()) as GLsizeiptr,
                self.text_colors.as_ptr() as *const GLvoid,
                gl::DYNAMIC_DRAW,
            );

            gl::DrawArrays(gl::TRIANGLES, 0, self.text_coords.len() as i32);
            gl::DisableVertexAttribArray(self.text_shader.attrib_v_color);
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

        let attrib_ptr = self.cursor_shader.attrib_apos;
        unsafe {
            gl::BindBuffer(gl::ARRAY_BUFFER, self.cursor_shader.vbo);

            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE);
            gl::BlendEquation(gl::FUNC_SUBTRACT);

            gl::BufferData(
                gl::ARRAY_BUFFER,
                (self.cursor_coords.len() * mem::size_of::<f32>()) as isize,
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

            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
            gl::BlendEquation(gl::FUNC_ADD);
        }
    }

    fn queue_text(&mut self, colors: Vec<&Color>, mut x: f32, mut y: f32, sx: f32, sy: f32) {
        let text = self.editor.text_all();
        let starting_x = x;

        // TODO: Cache this
        let mut coords: Vec<Point> = Vec::with_capacity(6 * text.len_chars());
        let mut colors_vertex: Vec<Color> = Vec::with_capacity(coords.capacity());

        let mut text_height = 0.0;
        let mut line_width = 0.0;

        for (i, ch) in text.chars().enumerate() {
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

            colors_vertex.push(colors[i].clone());
            colors_vertex.push(colors[i].clone());
            colors_vertex.push(colors[i].clone());
            colors_vertex.push(colors[i].clone());
            colors_vertex.push(colors[i].clone());
            colors_vertex.push(colors[i].clone());
        }

        // TODO: It's faster to directly mutate these vecs instead of making
        // new ones and replacing them. Also if we're only appending new text we don't need to
        // rebuild vecs in entirety
        self.text_coords = coords;
        self.text_colors = colors_vertex;

        self.text_height = text_height;
        self.text_width = self.text_width.max(line_width);
    }

    fn queue_highlights(&mut self) -> Vec<&'theme Color> {
        // TODO: Rope buffer is very inexpensive to clone (taking O(1) time),
        // so we should just do that here.
        let src: Vec<u8> = self.editor.text_all().bytes().collect();

        // Assume chars are 1 byte long (ascii)
        let mut text_colors: Vec<&Color> = vec![self.theme.fg(); src.len()];

        let highlights = self
            .highlighter
            .highlight(self.highlight_cfg, &src, None, |_| None)
            .unwrap();

        let mut color_stack: Vec<&Color> = Vec::new();

        for event in highlights {
            match event.unwrap() {
                HighlightEvent::Source { start, end } => {
                    if let Some(color) = color_stack.last() {
                        (start..end).for_each(|i| {
                            text_colors[i] = color;
                        });
                    }
                }
                HighlightEvent::HighlightStart(s) => {
                    if let Some(highlight) = Highlight::from_u8(s.0 as u8) {
                        color_stack.push(
                            self.theme
                                .highlight(highlight)
                                .unwrap_or_else(|| self.theme.fg()),
                        );
                    } else {
                        color_stack.push(self.theme.fg())
                    }
                }
                HighlightEvent::HighlightEnd => {
                    color_stack.pop();
                }
            }
        }

        text_colors
    }
}

impl<'theme, 'highlight> Window<'theme, 'highlight> {
    pub fn theme(&self) -> &ThemeType {
        self.theme
    }
}

pub struct TextShaderProgram {
    program: GLProgram,
    attrib_coord: GLuint,
    attrib_ytranslate: GLuint,
    attrib_xtranslate: GLuint,
    attrib_v_color: GLuint,
    uniform_tex: GLint,
    vbo: GLuint,
    vbo_color: GLuint,
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

        let mut vbo_color: GLuint = 0;
        unsafe {
            gl::GenBuffers(1, &mut vbo_color as *mut GLuint);
        }

        Self {
            attrib_coord: program.attrib("coord").unwrap() as u32,
            attrib_ytranslate: program.attrib("y_translate").unwrap() as u32,
            attrib_xtranslate: program.attrib("x_translate").unwrap() as u32,
            attrib_v_color: program.attrib("vertex_color").unwrap() as u32,
            uniform_tex: program.uniform("tex").unwrap(),
            vbo,
            vbo_color,
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
    vbo: GLuint,
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

        let mut vbo: GLuint = 0;
        unsafe { gl::GenBuffers(1, &mut vbo as *mut GLuint) }

        Self {
            attrib_apos: program.attrib("aPos").unwrap() as u32,
            attrib_ytranslate: program.attrib("y_translate").unwrap() as u32,
            attrib_xtranslate: program.attrib("x_translate").unwrap() as u32,
            uniform_time: program.uniform("time").unwrap(),
            uniform_laststroke: program.uniform("last_stroke").unwrap(),
            uniform_is_blinking: program.uniform("is_blinking").unwrap(),
            program,
            vbo,
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
