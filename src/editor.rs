use std::ops::Range;

use ropey::{Rope, RopeSlice};
use sdl2::{event::Event, keyboard::Keycode};

use crate::EventResult;

pub enum EditorEventResult {
    Nothing,
    DrawText,
    DrawCursor,
}

enum Mode {
    Insert,
    Normal,
}

pub struct Editor {
    // TODO: Represent text like this, vec of chars per line
    // text: Vec<Vec<char>>,
    cursor: usize,
    line: u32,
    text_pos: usize,
    text: Rope,
    mode: Mode,
}

impl Editor {
    pub fn new() -> Self {
        Self {
            cursor: 0,
            line: 0,
            text_pos: 0,
            text: Rope::new(),
            mode: Mode::Insert,
        }
    }

    pub fn event(&mut self, event: Event) -> EditorEventResult {
        match event {
            Event::KeyDown {
                keycode: Some(Keycode::H),
                ..
            } => {
                if let Mode::Normal = self.mode {
                    self.cursor -= 1;
                    EditorEventResult::DrawCursor
                } else {
                    EditorEventResult::Nothing
                }
            }
            Event::KeyDown {
                keycode: Some(Keycode::Escape),
                ..
            } => {
                self.mode = Mode::Normal;
                EditorEventResult::Nothing
            }
            Event::KeyDown {
                keycode: Some(Keycode::Backspace),
                ..
            } => {
                self.text.remove(self.text.len_chars() - 1..);
                self.cursor -= 1;
                self.text_pos -= 1;
                EditorEventResult::DrawText
            }
            Event::KeyDown {
                keycode: Some(Keycode::Return),
                ..
            } => {
                self.text.insert(self.text_pos, "\n");
                self.text_pos += 1;
                self.cursor = 0;
                self.line += 1;
                EditorEventResult::DrawText
            }
            Event::TextInput { text, .. } => {
                if let Mode::Insert = self.mode {
                    self.text.insert(self.text_pos, &text);
                    self.text_pos += 1;
                    self.cursor += text.len();
                    EditorEventResult::DrawText
                } else {
                    EditorEventResult::Nothing
                }
            }
            _ => EditorEventResult::Nothing,
        }
    }

    #[inline]
    pub fn text(&self, range: Range<usize>) -> RopeSlice {
        self.text.slice(range)
    }

    #[inline]
    pub fn text_all(&self) -> RopeSlice {
        self.text.slice(0..self.text.len_chars())
    }

    #[inline]
    pub fn line(&self) -> usize {
        self.line as usize
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.text.len_chars()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    pub fn cursor(&self) -> usize {
        self.cursor
    }
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}
