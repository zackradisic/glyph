use std::ops::Range;

use ropey::{Rope, RopeSlice};
use sdl2::{event::Event, keyboard::Keycode};

use crate::EditorEventResult;

#[derive(Copy, Clone)]
pub enum Mode {
    Insert,
    Normal,
}

/// CmdItem contains operators and operands for Vim commands
#[derive(Copy, Clone)]
enum CmdItem {
    Delete,
    Change,
    Find(bool),
    Char(char),
}

enum Movement {
    Relative(usize),
    Absolute(usize),
}

pub struct Editor {
    // In insert mode this is the next position to be written (1 + self.lines[line]).
    cursor: usize,
    line: usize,
    // TODO: Deleting/adding lines inbetween others is an O(n) operation, maybe be better to use the lines
    // provided by the rope buffer, this is has the trade off of always doing the O(logn) calculation, vs.
    // the O(1) access of a vec
    lines: Vec<u32>,
    text: Rope,
    mode: Mode,
    cmd_stack: Vec<CmdItem>,
}

fn text_to_lines(text: &str) -> Vec<u32> {
    let mut lines = Vec::new();

    let mut count = 0;
    let mut last = 'a';
    for c in text.chars() {
        last = c;
        if c == '\n' {
            lines.push(count);
            count = 0;
        } else {
            count += 1;
        }
    }

    if last != '\n' {
        lines.push(count);
    } else {
        lines.push(0);
    }

    lines
}

impl Editor {
    pub fn with_text(initial_text: Option<String>) -> Self {
        let (lines, text) = match initial_text {
            Some(text) => (text_to_lines(&text), Rope::from_str(&text)),
            None => (vec![0], Rope::new()),
        };
        Self {
            cursor: 0,
            lines,
            line: 0,
            text,
            mode: Mode::Insert,
            cmd_stack: Vec::with_capacity(3),
        }
    }

    pub fn new() -> Self {
        Editor::with_text(None)
    }

    pub fn event(&mut self, event: Event) -> EditorEventResult {
        // println!(
        //     "Abs={} Cursor={} Line={} Lines={:?}",
        //     self.pos(),
        //     self.cursor,
        //     self.line,
        //     self.lines
        // );
        match self.mode {
            Mode::Normal => match event {
                Event::KeyDown {
                    keycode: Some(Keycode::K),
                    ..
                } => {
                    self.up(1);
                    EditorEventResult::DrawCursor
                }
                Event::KeyDown {
                    keycode: Some(Keycode::J),
                    ..
                } => {
                    self.down(1);
                    EditorEventResult::DrawCursor
                }
                Event::KeyDown {
                    keycode: Some(Keycode::L),
                    ..
                } => {
                    if self.cursor + 1 < self.lines[self.line] as usize {
                        self.cursor += 1;
                        EditorEventResult::DrawCursor
                    } else {
                        EditorEventResult::Nothing
                    }
                }
                Event::KeyDown {
                    keycode: Some(Keycode::H),
                    ..
                } => self.left(1),
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
                        "A" => {
                            self.right(Movement::Absolute(self.lines[self.line] as usize));
                            self.mode = Mode::Insert;
                            return EditorEventResult::DrawText;
                        }
                        "a" => {
                            self.right(Movement::Relative(1));
                            self.mode = Mode::Insert;
                            return EditorEventResult::DrawText;
                        }
                        "c" => {
                            self.cmd_stack.push(CmdItem::Change);
                            return self.handle_cmd();
                        }
                        "d" => {
                            self.cmd_stack.push(CmdItem::Delete);
                            return self.handle_cmd();
                        }
                        "f" => {
                            self.cmd_stack.push(CmdItem::Find(true));
                            return self.handle_cmd();
                        }
                        "i" => self.mode = Mode::Insert,
                        "o" => {
                            self.new_line();
                            self.mode = Mode::Insert;
                            return EditorEventResult::DrawText;
                        }
                        "$" => {
                            self.cursor = self.lines[self.line] as usize - 1;
                            return EditorEventResult::DrawCursor;
                        }
                        s if s.chars().next().unwrap().is_alphabetic() => {
                            self.cmd_stack
                                .push(CmdItem::Char(s.chars().next().unwrap()));
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
                } => self.backspace(),
                Event::KeyDown {
                    keycode: Some(Keycode::Return),
                    ..
                } => {
                    self.enter();
                    EditorEventResult::DrawText
                }
                Event::TextInput { text, .. } => {
                    if let Mode::Insert = self.mode {
                        self.insert(&text);
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
        match self.cmd_stack.last().cloned() {
            Some(CmdItem::Find(forwards)) if self.cmd_stack.len() > 1 => {
                self.cmd_stack.pop();
                match self.cmd_stack.last().cloned() {
                    Some(CmdItem::Char(c)) => {
                        self.cmd_stack.pop();
                        if let Some(idx) = self.find_line(c, forwards) {
                            self.cursor = idx;
                        }
                        EditorEventResult::DrawCursor
                    }
                    Some(_) => {
                        self.cmd_stack.pop();
                        EditorEventResult::Nothing
                    }
                    None => unreachable!("We checked the stack had at least 2 cmds"),
                }
            }
            Some(CmdItem::Delete) if self.cmd_stack.len() > 1 => {
                self.cmd_stack.pop();
                match self.cmd_stack.last() {
                    Some(CmdItem::Delete) => {
                        self.cmd_stack.pop();
                        self.delete_line(self.line);
                        EditorEventResult::DrawText
                    }
                    Some(_) => {
                        self.cmd_stack.pop();
                        EditorEventResult::Nothing
                    }
                    None => unreachable!("We checked the vec had at least 2 cmds"),
                }
            }
            Some(CmdItem::Change) if self.cmd_stack.len() > 1 => {
                self.cmd_stack.pop();
                match self.cmd_stack.last() {
                    Some(CmdItem::Change) => {
                        self.cmd_stack.pop();
                        self.delete_line(self.line);
                        self.mode = Mode::Insert;
                        EditorEventResult::DrawText
                    }
                    Some(_) => {
                        self.cmd_stack.pop();
                        EditorEventResult::Nothing
                    }
                    None => unreachable!("We checked the vec had at least 2 cmds"),
                }
            }
            _ => EditorEventResult::Nothing,
        }
    }
}

// This impl contains text changing utilities
impl Editor {
    fn insert(&mut self, text: &str) {
        // TODO: This breaks if we move cursor beyond the lines we currently have,
        // should we not make it possible to move cursor beyond text like in vim?
        let pos = self.pos();

        self.text.insert(pos, text);
        self.cursor += text.len();
        self.lines[self.line] += 1;
    }

    fn backspace(&mut self) -> EditorEventResult {
        if self.cursor == 0 && self.line == 0 {
            return EditorEventResult::Nothing;
        }

        if self.text.len_chars() > 0 {
            self.text.remove(self.pos() - 1..self.pos());
        }
        self.cursor = if self.cursor > 0 {
            self.lines[self.line] -= 1;
            self.cursor - 1
        } else if self.line > 0 {
            // Backspacing into previous line
            let merge_line = self.lines.remove(self.line);
            self.line -= 1;
            self.lines[self.line] += merge_line;
            self.lines[self.line] as usize
        } else {
            0
        };
        EditorEventResult::DrawText
    }

    fn delete_line(&mut self, line: usize) {
        let pos = self.line_pos();
        if self.lines.len() > 1 {
            let len =
                // Include new line character, except if we one the last line which doesn't have it
                if line == (self.lines.len() - 1) { 0 } else { 1 } + self.lines.remove(line);

            self.text.remove(pos..(pos + len as usize))
        } else {
            self.lines[0] = 0;
            // Including \n from the last line
            self.text.remove(0..self.text.len_chars());
            self.cursor = 0;
        }
    }

    /// Insert a new line and splitting the current one based on the cursor position
    fn enter(&mut self) {
        let pos = self.pos();
        self.text.insert(pos, "\n");

        let new_line_count = self.lines[self.line] as usize - self.cursor;
        self.lines[self.line] = self.cursor as u32;

        self.line += 1;

        if self.line >= self.lines.len() {
            self.lines.push(new_line_count as u32)
        } else {
            self.lines.insert(self.line, new_line_count as u32);
        }

        self.cursor = 0;
    }

    // Insert a new line
    fn new_line(&mut self) {
        let pos = self.line_pos() + self.lines[self.line] as usize;
        self.text.insert(pos, "\n");
        self.cursor = 0;

        self.line += 1;

        if self.line >= self.lines.len() {
            self.lines.push(0)
        } else {
            self.lines.insert(self.line, 0);
        }
    }
}

// This impl contains movement utilities
impl Editor {
    #[inline]
    fn find_line(&mut self, char: char, forwards: bool) -> Option<usize> {
        if forwards {
            self.text
                .line(self.line)
                .chars()
                .skip(self.cursor)
                .enumerate()
                .find(|(_, c)| *c == char)
                .map(|tup| tup.0)
        } else {
            let mut chars = self.text.line(self.line).chars();
            chars.reverse();
            chars
                .skip(self.cursor)
                .enumerate()
                .find(|(_, c)| *c == char)
                .map(|tup| tup.0)
        }
    }

    #[inline]
    fn up(&mut self, count: usize) {
        if count > self.line {
            self.line = 0;
        } else {
            self.line -= count;
        }
        self.sync_line_cursor();
    }

    #[inline]
    fn down(&mut self, count: usize) {
        if self.line + count >= self.lines.len() {
            self.line = self.lines.len() - 1;
        } else {
            self.line += count;
        }
        self.sync_line_cursor();
    }

    #[inline]
    fn right(&mut self, movement: Movement) {
        match movement {
            Movement::Relative(count) => {
                let c = self.lines[self.line] as usize;
                if self.cursor + count > c {
                    self.cursor = if c == 0 { 0 } else { c - 1 };
                } else {
                    self.cursor += count;
                }
            }
            Movement::Absolute(pos) => {
                if pos > self.lines[self.line] as usize {
                    self.cursor = self.lines[self.line] as usize
                } else {
                    self.cursor = pos;
                }
            }
        }
    }

    #[inline]
    fn left(&mut self, count: usize) -> EditorEventResult {
        if self.cursor > 0 {
            self.cursor -= 1;
            EditorEventResult::DrawCursor
        } else {
            EditorEventResult::Nothing
        }
    }

    #[inline]
    fn sync_line_cursor(&mut self) {
        let line_count = self.lines[self.line] as usize;
        if line_count == 0 {
            self.cursor = 0;
        } else if self.cursor >= line_count {
            self.cursor = line_count - 1;
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
        if self.lines.len() == 1 {
            0
        } else {
            // Summation of every line before it + 1 for the new line character
            self.lines[0..self.line]
                .iter()
                .fold(0, |acc, line| acc + 1 + *line as usize)
        }
    }

    #[inline]
    pub fn is_insert(&self) -> bool {
        matches!(self.mode, Mode::Insert)
    }
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(test)]
    mod text_to_lines {
        use super::*;

        #[test]
        fn empty_line() {
            assert_eq!(vec![0], text_to_lines(""));
        }

        #[test]
        fn single_line() {
            let text = "one line";
            assert_eq!(vec![text.len() as u32], text_to_lines(text));
        }

        #[test]
        fn multiple_lines() {
            let text = "line 1\nline 2";
            assert_eq!(vec![6, 6], text_to_lines(text));
        }

        #[test]
        fn trailing_newline() {
            let text = "line 1\n";
            assert_eq!(vec![6, 0], text_to_lines(text));
        }

        #[test]
        fn leading_newline() {
            let text = "\nline 1\n";
            assert_eq!(vec![0, 6, 0], text_to_lines(text));
        }
    }

    #[cfg(test)]
    mod movement {
        use super::*;

        #[test]
        fn sync_lines() {
            // Should not exceed line length
            let mut editor = Editor::new();
            editor.insert("1");
            editor.insert("2");
            editor.enter();
            editor.insert("1");
            editor.insert("2");
            editor.insert("3");
            editor.up(1);

            assert_eq!(editor.cursor, 1);
        }
    }

    #[cfg(test)]
    mod edit {
        use super::*;

        #[test]
        fn delete_line_first() {
            let mut editor = Editor::new();
            editor.insert("1");
            editor.enter();
            editor.insert("1");
            editor.enter();
            editor.insert("1");
            editor.up(2);
            editor.delete_line(0);

            assert_eq!(editor.lines, vec![1, 1]);
        }

        #[test]
        fn delete_line_middle() {
            let mut editor = Editor::new();
            editor.insert("1");
            editor.enter();
            editor.insert("1");
            editor.insert("1");
            editor.insert("1");
            editor.enter();
            editor.insert("1");
            editor.insert("2");
            editor.up(1);
            editor.delete_line(1);

            assert_eq!(editor.lines, vec![1, 2]);
        }

        #[test]
        fn delete_line_last() {
            let mut editor = Editor::new();
            editor.insert("1");
            editor.enter();
            editor.insert("1");
            editor.enter();
            editor.insert("1");
            editor.insert("2");
            editor.delete_line(2);

            assert_eq!(editor.lines, vec![1, 1]);
        }

        #[test]
        fn backspace_beginning_in_between_line() {
            let mut editor = Editor::new();
            editor.insert("1");
            editor.insert("2");
            editor.insert("3");
            editor.enter();
            editor.insert("1");
            editor.enter();
            editor.insert("1");
            editor.up(1);
            editor.left(1);

            assert_eq!(editor.backspace(), EditorEventResult::DrawText);
            assert_eq!(editor.lines, vec![4, 1]);
        }

        #[test]
        fn enter_in_between() {
            let mut editor = Editor::new();
            editor.insert("1");
            editor.insert("2");
            editor.insert("3");
            editor.cursor = 2;

            editor.enter();
            assert_eq!(editor.lines, vec![2, 1]);
        }

        #[test]
        fn enter_beginning() {
            let mut editor = Editor::new();
            editor.insert("1");
            editor.insert("2");
            editor.insert("3");
            editor.cursor = 0;

            editor.enter();
            assert_eq!(editor.lines, vec![0, 3]);
        }

        #[test]
        fn enter_end() {
            let mut editor = Editor::new();
            editor.insert("1");
            editor.insert("2");
            editor.insert("3");
            editor.cursor = 3;

            editor.enter();
            assert_eq!(editor.lines, vec![3, 0]);
        }
    }
}
