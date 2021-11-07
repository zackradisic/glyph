use std::ops::Range;

use ropey::{Rope, RopeSlice};
use sdl2::{event::Event, keyboard::Keycode, sys::KeyCode};

use crate::EventResult;

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
            line: 1,
            text_pos: 0,
            text: Rope::new(),
            mode: Mode::Insert,
        }
    }

    pub fn event(&mut self, event: Event) -> EventResult {
        match event {
            Event::KeyDown {
                keycode: Some(Keycode::Backspace),
                ..
            } => {
                self.text.remove(self.text.len_chars() - 1..);
                // self.cursor -= 1;
                self.text_pos -= 1;
                EventResult::Draw
            }
            Event::KeyDown {
                keycode: Some(Keycode::Return),
                ..
            } => {
                self.text.insert(self.text_pos, "\n");
                self.text_pos += 1;
                // self.cursor = 0;
                self.line += 1;
                EventResult::Draw
            }
            Event::TextInput { text, .. } => {
                self.text.insert(self.text_pos, &text);
                self.text_pos += 1;
                // self.cursor += text.len();
                EventResult::Draw
            }
            _ => EventResult::Nothing,
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
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}
