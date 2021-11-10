use std::ops::Range;

use ropey::{Rope, RopeSlice};
use sdl2::{event::Event, keyboard::Keycode};

pub enum EditorEventResult {
    Nothing,
    DrawText,
    DrawCursor,
}

enum Mode {
    Insert,
    Normal,
}

enum Cmd {
    Delete,
}

pub struct Editor {
    cursor: usize,
    line: usize,
    // TODO: Deleting lines is O(n), maybe be better to use the lines provided
    // by the rope buffer, this is has the trade off of O(logn) accesses over O(1) of the vec
    lines: Vec<u32>,
    text: Rope,
    mode: Mode,
    cmd_stack: Vec<Cmd>,
}

impl Editor {
    pub fn new() -> Self {
        Self {
            cursor: 0,
            lines: vec![0],
            line: 0,
            text: Rope::new(),
            mode: Mode::Insert,
            cmd_stack: Vec::with_capacity(3),
        }
    }

    pub fn event(&mut self, event: Event) -> EditorEventResult {
        println!(
            "Cursor={} Line={} Lines={:?}",
            self.cursor, self.line, self.lines
        );
        match self.mode {
            Mode::Normal => match event {
                Event::KeyDown {
                    keycode: Some(Keycode::K),
                    ..
                } => {
                    if let Mode::Normal = self.mode {
                        self.line -= 1;
                        EditorEventResult::DrawCursor
                    } else {
                        EditorEventResult::Nothing
                    }
                }
                Event::KeyDown {
                    keycode: Some(Keycode::J),
                    ..
                } => {
                    if let Mode::Normal = self.mode {
                        self.line += 1;
                        EditorEventResult::DrawCursor
                    } else {
                        EditorEventResult::Nothing
                    }
                }
                Event::KeyDown {
                    keycode: Some(Keycode::L),
                    ..
                } => {
                    if let Mode::Normal = self.mode {
                        self.cursor += 1;
                        EditorEventResult::DrawCursor
                    } else {
                        EditorEventResult::Nothing
                    }
                }
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
                    keycode: Some(Keycode::O),
                    ..
                } => {
                    self.new_line();
                    self.mode = Mode::Insert;
                    EditorEventResult::DrawText
                }
                Event::KeyDown {
                    keycode: Some(Keycode::Num0) | Some(Keycode::Kp0),
                    ..
                } => {
                    self.cursor = 0;
                    EditorEventResult::DrawCursor
                }
                // Handle TextInput instead of KeyDown for mode switch because both are sent,
                // but TextInput comes last. If you handle KeyDown first then insert mode will
                // process 'i' and add it to text
                Event::TextInput { text, .. } => {
                    match text.as_str() {
                        "i" => self.mode = Mode::Insert,
                        "$" => {
                            self.cursor = self.lines[self.line] as usize;
                            return EditorEventResult::DrawCursor;
                        }
                        "d" => {
                            self.cmd_stack.push(Cmd::Delete);
                            return self.handle_cmd();
                        }
                        _ => {}
                    }
                    EditorEventResult::Nothing
                }
                _ => EditorEventResult::Nothing,
            },
            Mode::Insert => match event {
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
                    if self.text.len_chars() > 0 {
                        self.text.remove(self.text.len_chars() - 1..);
                    }
                    self.cursor = if self.cursor > 0 {
                        self.lines[self.line] -= 1;
                        self.cursor - 1
                    } else if self.line > 0 {
                        self.line -= 1;
                        self.lines[self.line] -= 1;
                        self.lines[self.line] as usize
                    } else {
                        0
                    };
                    EditorEventResult::DrawText
                }
                Event::KeyDown {
                    keycode: Some(Keycode::Return),
                    ..
                } => {
                    self.new_line();
                    EditorEventResult::DrawText
                }
                Event::TextInput { text, .. } => {
                    if let Mode::Insert = self.mode {
                        // TODO: This breaks if we move cursor beyond the lines we currently have,
                        // should we not make it possible to move cursor beyond text like in vim?
                        let pos = self.pos();

                        // If we we moved cursor beyond text and inserting, fill the space
                        // with spaces
                        let len = self.text.len_chars();
                        if pos > len {
                            let amount = pos - len;
                            (0..amount).for_each(|i| self.text.insert_char(len + i, ' '));
                            self.lines[self.line] += amount as u32;
                        }

                        self.text.insert(pos, &text);
                        self.cursor += text.len();
                        self.lines[self.line] += 1;

                        EditorEventResult::DrawText
                    } else {
                        EditorEventResult::Nothing
                    }
                }
                _ => EditorEventResult::Nothing,
            },
        }
    }

    fn handle_cmd(&mut self) -> EditorEventResult {
        match self.cmd_stack.last() {
            Some(Cmd::Delete) if self.cmd_stack.len() > 1 => {
                self.cmd_stack.pop();
                match self.cmd_stack.last() {
                    Some(Cmd::Delete) => {
                        self.cmd_stack.pop();
                        // TODO: We should emulate vim behaviour here,
                        // if we are on last line move cursor up
                        self.delete_line(self.line);
                        EditorEventResult::DrawText
                    }
                    _ => unreachable!("We checked the vec had at least 2 cmds"),
                }
            }
            _ => EditorEventResult::Nothing,
        }
    }
}

// This impl contains utilities to change the text
impl Editor {
    fn delete_line(&mut self, line: usize) {
        // TODO: This is O(n), could be bad for performance
        let pos = self.line_pos();
        let len = self.lines.remove(line);
        if line > 0 {
            self.text.remove(pos..(pos + len as usize))
        } else {
            // Including \n from the last line
            self.text.remove((pos - 1)..(pos + len as usize))
        }
    }

    fn new_line(&mut self) {
        self.text.insert(self.pos(), "\n");
        self.cursor = 0;
        self.lines[self.line] += 1;
        self.line += 1;
        if self.line >= self.lines.len() {
            self.lines.push(0)
        }
    }
}

// This impl contains small utility functions
impl Editor {
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
    fn len(&self) -> usize {
        self.text.len_chars()
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    #[inline]
    fn pos(&self) -> usize {
        self.line_pos() + self.cursor
    }

    #[inline]
    fn line_pos(&self) -> usize {
        self.lines[0..self.line]
            .iter()
            .fold(0, |acc, line| acc + *line as usize)
    }
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}
