use std::{
    ffi::{c_void, CString},
    mem,
    ptr::null,
    sync::{Arc, RwLock},
};

use gl::types::{GLint, GLsizeiptr, GLuint, GLvoid};
use lsp::{Client, Diagnostics, LspSender};
use once_cell::sync::Lazy;
use sdl2::{
    event::Event,
    keyboard::{Keycode, Mod},
};
use syntax::tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};
use syntax::Highlight;

use crate::{
    atlas::Atlas, Color, Editor, EditorEvent, EventResult, GLProgram, Shader, ThemeType,
    WindowFrameKind, ERROR_RED, SCREEN_HEIGHT, SCREEN_WIDTH,
};

#[repr(C)]
struct Point {
    x: f32,
    y: f32,
    s: f32,
    t: f32,
}
#[derive(Clone, Debug)]
#[repr(C)]
struct Point3 {
    // x == f32::MAX signifies Point3 is null
    x: f32,
    y: f32,
    z: f32,
}

impl Point3 {
    #[inline]
    fn is_null(&self) -> bool {
        self.x == f32::MAX
    }

    #[inline]
    fn null() -> Point3 {
        Point3 {
            x: f32::MAX,
            y: f32::MAX,
            z: f32::MAX,
        }
    }
}

impl Default for Point3 {
    fn default() -> Self {
        Self {
            x: f32::MAX,
            y: Default::default(),
            z: Default::default(),
        }
    }
}

const SX: f32 = 0.8 / SCREEN_WIDTH as f32;
const SY: f32 = 0.8 / SCREEN_HEIGHT as f32;

const START_X: f32 = -1f32 + 8f32 * SX;
const START_Y: f32 = 1f32 - 50f32 * SY;

pub struct Window<'theme, 'highlight> {
    // Graphics
    atlas: Atlas,
    text_shader: TextShaderProgram,
    cursor_shader: CursorShaderProgram,
    highlight_shader: HighlightShaderProgram,
    diagnostic_shader: DiagnosticShaderProgram,
    editor: Editor,
    text_coords: Vec<Point>,
    text_colors: Vec<Color>,
    cursor_coords: [Point3; 6],
    highlight_coords: Vec<Point3>,
    diagnostics_coords: Vec<Point3>,
    diagnostics_colors: Vec<Color>,
    y_offset: f32,
    x_offset: f32,
    text_height: f32,
    text_width: f32,
    last_stroke: u32, // Time since last stroke in ms

    // Syntax highlighting
    theme: &'theme ThemeType,
    highlighter: Highlighter,
    highlight_cfg: &'highlight Lazy<HighlightConfiguration>,
    text_changed: bool,
    cursor_changed: bool,

    // LSP
    diagnostics: Option<Arc<RwLock<Diagnostics>>>,
    lsp_send: Option<LspSender>,
    last_clock: u64,
}

impl<'theme, 'highlight> Window<'theme, 'highlight> {
    pub fn new(
        initial_text: Option<String>,
        theme: &'theme ThemeType,
        lsp_client: Option<&Client>,
    ) -> Self {
        let font_path = "./fonts/FiraCode.ttf";

        let text_shader = TextShaderProgram::default();
        let atlas = Atlas::new(font_path, 48, text_shader.uniform_tex).unwrap();
        let cursor_shader = CursorShaderProgram::default();
        let highlight_shader = HighlightShaderProgram::default();
        let diagnostic_shader = DiagnosticShaderProgram::default();

        let highlighter = Highlighter::new();

        let mut editor = Editor::with_text(initial_text);
        if let Some(lsp_client) = lsp_client {
            editor.configure_lsp(lsp_client);
        }

        Self {
            atlas,
            text_shader,
            cursor_shader,
            highlight_shader,
            diagnostic_shader,
            editor,
            text_coords: Vec::new(),
            text_colors: Vec::new(),
            cursor_coords: Default::default(),
            highlight_coords: Default::default(),
            diagnostics_coords: Default::default(),
            diagnostics_colors: Vec::new(),
            y_offset: 0.0,
            x_offset: 0.0,
            text_height: 0.0,
            text_width: 0.0,
            last_stroke: 0,

            theme,
            highlighter,
            highlight_cfg: &syntax::RUST_CFG,
            text_changed: false,
            cursor_changed: false,

            diagnostics: lsp_client.map(|c| c.diagnostics().clone()),
            lsp_send: lsp_client.map(|c| c.sender().clone()),
            last_clock: 0,
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
                    self.scroll_y(y as f32);
                }
                self.queue_cursor();
                EventResult::Scroll
            }
            _ => {
                let evt = self.editor.event(event);
                self.handle_editor_event(evt, time)
            }
        }
    }
}

// This impl contains utilities
impl<'theme, 'highlight> Window<'theme, 'highlight> {
    fn scroll_y(&mut self, mut amount: f32) {
        let pix_amount = amount * self.atlas.max_h;
        amount *= -1.0;
        match pix_amount > 0.0 {
            // Scrolling up
            true => {
                if self.y_offset + pix_amount >= 0.0 {
                    self.y_offset = 0.0;
                } else {
                    self.y_offset += pix_amount;
                    self.editor.incr_line(amount as i32)
                }
            }
            // Scrolling down
            false => {
                if -1.0 * (self.y_offset + pix_amount) >= self.text_height {
                    self.y_offset = self.text_height * -1.0;
                    let len = self.editor.lines().len();
                    if len == 0 {
                        self.editor.incr_line(0)
                    } else {
                        self.editor.set_line(len - 1);
                    }
                } else {
                    self.y_offset += pix_amount;
                    self.editor.incr_line(amount as i32)
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
    #[inline]
    fn handle_editor_event(&mut self, evt: EditorEvent, time: u32) -> EventResult {
        match evt {
            EditorEvent::DrawText => {
                self.text_changed = true;
                self.last_stroke = time;
                self.render_text();
                EventResult::Draw
            }
            EditorEvent::DrawCursor => {
                self.cursor_changed = true;
                self.adjust_scroll();
                self.queue_cursor();
                EventResult::Draw
            }
            EditorEvent::DrawSelection => {
                self.queue_selection(START_X, START_Y, SX, SY);
                EventResult::Draw
            }
            EditorEvent::Multiple => {
                let evts = self.editor.take_multiple_event_data();
                let mut draw = false;

                for evt in evts.into_iter() {
                    if matches!(self.handle_editor_event(evt, time), EventResult::Draw) {
                        draw = true;
                    }
                }

                if draw {
                    EventResult::Draw
                } else {
                    EventResult::Nothing
                }
            }

            _ => EventResult::Nothing,
        }
    }

    pub fn render_text(&mut self) {
        self.adjust_scroll();
        self.queue_cursor();
        let colors = self.queue_highlights();
        self.queue_text(colors, -1f32 + 8f32 * SX, 1f32 - 50f32 * SY, SX, SY);
        self.queue_selection(-1f32 + 8f32 * SX, 1f32 - 50f32 * SY, SX, SY)
    }

    pub fn queue_cursor(&mut self) {
        let w = self.atlas.max_w * SX;
        let real_h = self.atlas.max_h * SY;
        let h = (self.atlas.max_h/*+ 5f32*/) * SY;

        let x = (-1f32 + 8f32 * SX)
            + (self.editor.cursor() as f32 * (w/*+ self.atlas.glyphs[35].advance_x * SX*/));
        let y = ((1f32 - 50f32 * SY) + real_h) - (self.editor.line() as f32 * real_h);

        self.cursor_coords = [
            // // bottom left
            Point3 {
                x,
                y: y - h,
                z: 0.0,
            },
            // top left
            Point3 { x, y, z: 0.0 },
            // top right
            Point3 {
                x: x + w,
                y,
                z: 0.0,
            },
            // bottom right
            Point3 {
                x: x + w,
                y: y - h,
                z: 0.0,
            },
            // top right,
            Point3 {
                x: x + w,
                y,
                z: 0.0,
            },
            // bottom leff
            Point3 {
                x,
                y: y - h,
                z: 0.0,
            },
        ];
    }

    pub fn frame(&mut self, kind: WindowFrameKind, ticks_ms: u32) {
        let draw = matches!(kind, WindowFrameKind::Draw);
        self.text_shader.set_used();

        // Draw text
        unsafe {
            // TODO: X and Y translation can be global (make it a uniform)
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
            if draw {
                gl::BufferData(
                    gl::ARRAY_BUFFER,
                    (self.text_coords.len() * mem::size_of::<Point>()) as GLsizeiptr,
                    self.text_coords.as_ptr() as *const GLvoid,
                    gl::DYNAMIC_DRAW,
                );
            }

            gl::BindBuffer(gl::ARRAY_BUFFER, self.text_shader.vbo_color);
            gl::VertexAttribPointer(
                self.text_shader.attrib_v_color,
                4,
                gl::UNSIGNED_BYTE,
                gl::TRUE,
                0,
                null(),
            );
            gl::EnableVertexAttribArray(self.text_shader.attrib_v_color);
            if draw {
                gl::BufferData(
                    gl::ARRAY_BUFFER,
                    (self.text_colors.len() * mem::size_of::<Color>()) as GLsizeiptr,
                    self.text_colors.as_ptr() as *const GLvoid,
                    gl::DYNAMIC_DRAW,
                );
            }

            gl::DrawArrays(gl::TRIANGLES, 0, self.text_coords.len() as i32);
            gl::DisableVertexAttribArray(self.text_shader.attrib_v_color);
            gl::DisableVertexAttribArray(self.text_shader.attrib_coord);
        }

        // Draw highlight
        {
            self.highlight_shader.set_used();
            let attrib_ptr = self.highlight_shader.attrib_apos;
            unsafe {
                gl::VertexAttrib1f(self.highlight_shader.attrib_ytranslate, self.y_offset * SY);
                gl::VertexAttrib1f(self.highlight_shader.attrib_xtranslate, self.x_offset * SX);

                gl::BindBuffer(gl::ARRAY_BUFFER, self.highlight_shader.vbo);
                if draw {
                    gl::BufferData(
                        gl::ARRAY_BUFFER,
                        (self.highlight_coords.len() * mem::size_of::<Point3>()) as isize,
                        self.highlight_coords.as_ptr() as *const c_void,
                        gl::DYNAMIC_DRAW,
                    );
                }
                gl::VertexAttribPointer(
                    attrib_ptr,
                    3,
                    gl::FLOAT,
                    gl::FALSE,
                    mem::size_of::<Point3>() as i32,
                    null(),
                );
                gl::EnableVertexAttribArray(0);

                gl::DrawArrays(gl::TRIANGLES, 0, self.highlight_coords.len() as i32);
                gl::DisableVertexAttribArray(0);
            }
        }
        // Draw diagnostics
        {
            self.diagnostic_shader.set_used();
            unsafe {
                gl::VertexAttrib1f(self.diagnostic_shader.attrib_ytranslate, self.y_offset * SY);
                gl::VertexAttrib1f(self.diagnostic_shader.attrib_xtranslate, self.x_offset * SX);

                // Coords
                gl::BindBuffer(gl::ARRAY_BUFFER, self.diagnostic_shader.vbo);
                if draw {
                    gl::BufferData(
                        gl::ARRAY_BUFFER,
                        (self.diagnostics_coords.len() * mem::size_of::<Point3>()) as isize,
                        self.diagnostics_coords.as_ptr() as *const c_void,
                        gl::DYNAMIC_DRAW,
                    );
                }
                gl::VertexAttribPointer(
                    self.diagnostic_shader.attrib_apos,
                    3,
                    gl::FLOAT,
                    gl::FALSE,
                    mem::size_of::<Point3>() as i32,
                    null(),
                );
                // Color
                gl::BindBuffer(gl::ARRAY_BUFFER, self.diagnostic_shader.vbo_color);
                if draw {
                    gl::BufferData(
                        gl::ARRAY_BUFFER,
                        (self.diagnostics_colors.len() * mem::size_of::<Color>()) as isize,
                        self.diagnostics_colors.as_ptr() as *const c_void,
                        gl::DYNAMIC_DRAW,
                    );
                }
                gl::VertexAttribPointer(
                    self.diagnostic_shader.attrib_color,
                    4,
                    gl::UNSIGNED_BYTE,
                    gl::TRUE,
                    0,
                    null(),
                );

                gl::EnableVertexAttribArray(self.diagnostic_shader.attrib_apos);
                gl::EnableVertexAttribArray(self.diagnostic_shader.attrib_color);
                gl::DrawArrays(gl::TRIANGLES, 0, self.diagnostics_coords.len() as i32);
                gl::DisableVertexAttribArray(self.diagnostic_shader.attrib_apos);
                gl::DisableVertexAttribArray(self.diagnostic_shader.attrib_color);
            }
        }

        // Draw cursor
        {
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

                // gl::BlendFunc(gl::SRC_ALPHA, gl::ONE);
                // gl::BlendEquation(gl::FUNC_SUBTRACT);

                if draw {
                    gl::BufferData(
                        gl::ARRAY_BUFFER,
                        (self.cursor_coords.len() * mem::size_of::<Point3>()) as isize,
                        self.cursor_coords.as_ptr() as *const c_void,
                        gl::DYNAMIC_DRAW,
                    );
                }

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
    }

    pub fn queue_diagnostics(&mut self) {
        if let Some(diagnostics) = &self.diagnostics {
            let d = diagnostics.read().unwrap();
            if self.last_clock != d.clock {
                let mut coords: Vec<Point3> = Vec::new();
                let mut colors: Vec<Color> = Vec::new();

                let mut col = 0;
                for diag in &d.diagnostics {
                    let max_w = self.atlas.max_w * SX;
                    let max_h = self.atlas.max_h * SY;

                    let mut x = START_X;
                    let mut y = START_Y;

                    let mut top_left: Point3 = Point3::null();
                    let mut bot_left: Point3 = Point3::null();

                    let lsp::Range {
                        start: start_pos,
                        end: end_pos,
                    } = diag.range;
                    let start = self.editor.line_idx(start_pos.line as usize);

                    let end = self
                        .editor
                        .line_char_idx(end_pos.line as usize, end_pos.character as usize);

                    let within_range = |i: usize| -> bool {
                        (i + start) >= (start + start_pos.character as usize) && (i + start) < end
                    };

                    y -= max_h * (start_pos.line as f32);

                    for (i, ch) in self.editor.text(start..(end + 1)).chars().enumerate() {
                        let c = ch as usize;

                        // Calculate the vertex and texture coordinates
                        let x2 = x + (col as f32 * max_w);
                        // let x2 = x + max_w;
                        let y2 = -y;
                        let width = self.atlas.glyphs[c].bitmap_w * SX;
                        let height = self.atlas.glyphs[c].bitmap_h * SY;

                        // Skip glyphs that have no pixels
                        if (width == 0.0 || height == 0.0) && !within_range(i) {
                            match ch as u8 {
                                32 => {
                                    col += 1;
                                }
                                // Tab
                                9 => {
                                    x += self.atlas.max_w * SY * 4f32;
                                    col += 4;
                                }
                                // New line
                                10 => {
                                    y -= max_h;
                                    x = START_X;
                                    if !top_left.is_null() {
                                        let bot_right = Point3 {
                                            x: x2,
                                            y: -y2 + max_h,
                                            z: 0.0,
                                        };
                                        let top_right = Point3 {
                                            x: x2,
                                            y: -y2,
                                            z: 0.0,
                                        };
                                        // First triangle
                                        coords.push(top_left.clone());
                                        coords.push(bot_left.clone());
                                        coords.push(bot_right.clone());
                                        // Second triangle
                                        coords.push(top_left.clone());
                                        coords.push(top_right);
                                        coords.push(bot_right);
                                        colors.push(ERROR_RED);
                                        colors.push(ERROR_RED);
                                        colors.push(ERROR_RED);
                                        colors.push(ERROR_RED);
                                        colors.push(ERROR_RED);
                                        colors.push(ERROR_RED);

                                        top_left = Point3::null();
                                        bot_left = Point3::null();
                                    }
                                    col = 0;
                                }
                                _ => {}
                            }
                            continue;
                        }

                        if top_left.is_null() && within_range(i) {
                            top_left = Point3 {
                                x: x2,
                                y: -y2,
                                z: 0.0,
                            };
                            bot_left = Point3 {
                                x: x2,
                                y: -y2 + max_h,
                                z: 0.0,
                            };
                        } else if !top_left.is_null() && !within_range(i) {
                            let bot_right = Point3 {
                                x: x2,
                                y: -y2 + max_h,
                                z: 0.0,
                            };
                            let top_right = Point3 {
                                x: x2,
                                y: -y2,
                                z: 0.0,
                            };
                            // First triangle
                            coords.push(top_left.clone());
                            coords.push(bot_left.clone());
                            coords.push(bot_right.clone());
                            // Second triangle
                            coords.push(top_left.clone());
                            coords.push(top_right);
                            coords.push(bot_right);
                            colors.push(ERROR_RED);
                            colors.push(ERROR_RED);
                            colors.push(ERROR_RED);
                            colors.push(ERROR_RED);
                            colors.push(ERROR_RED);
                            colors.push(ERROR_RED);
                            break;
                        } else if i + start >= end {
                            if !top_left.is_null() {
                                let bot_right = Point3 {
                                    x: x2,
                                    y: -y2 + max_h,
                                    z: 0.0,
                                };
                                let top_right = Point3 {
                                    x: x2,
                                    y: -y2,
                                    z: 0.0,
                                };
                                // First triangle
                                coords.push(top_left.clone());
                                coords.push(bot_left.clone());
                                coords.push(bot_right.clone());
                                // Second triangle
                                coords.push(top_left.clone());
                                coords.push(top_right);
                                coords.push(bot_right);
                                colors.push(ERROR_RED);
                                colors.push(ERROR_RED);
                                colors.push(ERROR_RED);
                                colors.push(ERROR_RED);
                                colors.push(ERROR_RED);
                                colors.push(ERROR_RED);
                            }
                            break;
                        }
                        col += 1;
                    }
                }

                self.diagnostics_coords = coords;
                self.diagnostics_colors = colors;
                self.last_clock = d.clock;
            }
        }
    }

    fn queue_selection(&mut self, mut x: f32, mut y: f32, sx: f32, sy: f32) {
        if self.editor.selection().is_none() {
            self.highlight_coords.clear();
            return;
        }

        let mut hl_coords: Vec<Point3> = Vec::new();

        let starting_x = x;
        let max_w = self.atlas.max_w * sx;
        let max_h = self.atlas.max_h * sy;

        let mut top_left: Point3 = Point3::null();
        let mut bot_left: Point3 = Point3::null();

        let mut col: u32 = 0;
        for (i, ch) in self.editor.text_all().chars().enumerate() {
            let c = ch as usize;

            // Calculate the vertex and texture coordinates
            let x2 = x + (col as f32 * max_w);
            // let x2 = x + max_w;
            let y2 = -y;
            let width = self.atlas.glyphs[c].bitmap_w * sx;
            let height = self.atlas.glyphs[c].bitmap_h * sy;

            // Skip glyphs that have no pixels
            if (width == 0.0 || height == 0.0) && !self.editor.past_selection(i as u32) {
                match ch as u8 {
                    32 => {
                        col += 1;
                    }
                    // Tab
                    9 => {
                        x += self.atlas.max_w * sy * 4f32;
                        col += 4;
                    }
                    // New line
                    10 => {
                        y -= max_h;
                        x = starting_x;
                        if !top_left.is_null() {
                            let bot_right = Point3 {
                                x: x2,
                                y: -y2 + max_h,
                                z: 0.0,
                            };
                            let top_right = Point3 {
                                x: x2,
                                y: -y2,
                                z: 0.0,
                            };
                            // First triangle
                            hl_coords.push(top_left.clone());
                            hl_coords.push(bot_left.clone());
                            hl_coords.push(bot_right.clone());
                            // Second triangle
                            hl_coords.push(top_left.clone());
                            hl_coords.push(top_right);
                            hl_coords.push(bot_right);

                            top_left = Point3::null();
                            bot_left = Point3::null();
                        }
                        col = 0;
                    }
                    _ => {}
                }
                continue;
            }

            if top_left.is_null() && self.editor.within_selection(i as u32) {
                top_left = Point3 {
                    x: x2,
                    y: -y2,
                    z: 0.0,
                };
                bot_left = Point3 {
                    x: x2,
                    y: -y2 + max_h,
                    z: 0.0,
                };
            } else if !top_left.is_null() && !self.editor.within_selection(i as u32) {
                let bot_right = Point3 {
                    x: x2,
                    y: -y2 + max_h,
                    z: 0.0,
                };
                let top_right = Point3 {
                    x: x2,
                    y: -y2,
                    z: 0.0,
                };
                // First triangle
                hl_coords.push(top_left.clone());
                hl_coords.push(bot_left.clone());
                hl_coords.push(bot_right.clone());
                // Second triangle
                hl_coords.push(top_left.clone());
                hl_coords.push(top_right);
                hl_coords.push(bot_right);
                break;
            } else if self.editor.past_selection(i as u32) {
                break;
            }
            col += 1;
        }

        self.highlight_coords = hl_coords;
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
                match ch as u8 {
                    // Tab
                    9 => {
                        x += self.atlas.max_w * sy * 4f32;
                    }
                    // New line
                    10 => {
                        y -= self.atlas.max_h * sy;
                        text_height += self.atlas.max_h;
                        self.text_height = self.text_height.max(text_height);
                        line_width = 0.0;
                        x = starting_x;
                    }
                    _ => {}
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

            colors_vertex.push(*colors[i]);
            colors_vertex.push(*colors[i]);
            colors_vertex.push(*colors[i]);
            colors_vertex.push(*colors[i]);
            colors_vertex.push(*colors[i]);
            colors_vertex.push(*colors[i]);
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

    fn adjust_scroll(&mut self) {
        let oy = self.line_y_offset(self.editor.line());
        let scrolled_h = SCREEN_HEIGHT as f32 * 2.0 + (self.y_offset * -1.0);

        // Multiply by two because retina display on Mac
        if oy >= scrolled_h || oy < self.y_offset * -1.0 {
            self.y_offset = oy * -1.0;
        }
    }
}

// This impl contains small utilities
impl<'theme, 'highlight> Window<'theme, 'highlight> {
    pub fn theme(&self) -> &ThemeType {
        self.theme
    }

    // Get the y offset (scroll pos) for the given line
    #[inline]
    fn line_y_offset(&self, line: usize) -> f32 {
        (self.atlas.max_h as f32 * line as f32) - START_Y
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

pub struct HighlightShaderProgram {
    program: GLProgram,
    attrib_ytranslate: GLuint,
    attrib_xtranslate: GLuint,
    attrib_apos: GLuint,
    vbo: GLuint,
}

impl HighlightShaderProgram {
    pub fn new() -> Self {
        let shaders = vec![
            Shader::from_source(
                &CString::new(include_str!("../shaders/highlight.v.glsl")).unwrap(),
                gl::VERTEX_SHADER,
            )
            .unwrap(),
            Shader::from_source(
                &CString::new(include_str!("../shaders/highlight.f.glsl")).unwrap(),
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
            program,
            vbo,
        }
    }

    #[inline]
    pub fn set_used(&self) {
        self.program.set_used()
    }
}

impl Default for HighlightShaderProgram {
    fn default() -> Self {
        Self::new()
    }
}

pub struct DiagnosticShaderProgram {
    program: GLProgram,
    attrib_color: GLuint,
    attrib_ytranslate: GLuint,
    attrib_xtranslate: GLuint,
    attrib_apos: GLuint,
    vbo: GLuint,
    vbo_color: GLuint,
}

impl DiagnosticShaderProgram {
    pub fn new() -> Self {
        let shaders = vec![
            Shader::from_source(
                &CString::new(include_str!("../shaders/diagnostic.v.glsl")).unwrap(),
                gl::VERTEX_SHADER,
            )
            .unwrap(),
            Shader::from_source(
                &CString::new(include_str!("../shaders/diagnostic.f.glsl")).unwrap(),
                gl::FRAGMENT_SHADER,
            )
            .unwrap(),
        ];

        let program = GLProgram::from_shaders(&shaders).unwrap();

        let mut vbo = 0;
        let mut vbo_color = 0;
        unsafe {
            gl::GenBuffers(1, &mut vbo as *mut GLuint);
            gl::GenBuffers(1, &mut vbo_color as *mut GLuint);
        }

        Self {
            attrib_apos: program.attrib("aPos").unwrap() as u32,
            attrib_color: program.attrib("vertex_color").unwrap() as u32,
            attrib_ytranslate: program.attrib("y_translate").unwrap() as u32,
            attrib_xtranslate: program.attrib("x_translate").unwrap() as u32,
            program,
            vbo,
            vbo_color,
        }
    }

    #[inline]
    pub fn set_used(&self) {
        self.program.set_used()
    }
}

impl Default for DiagnosticShaderProgram {
    fn default() -> Self {
        Self::new()
    }
}
