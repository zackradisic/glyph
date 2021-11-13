use std::{cmp::Ordering, ops::Range};

use ropey::{Rope, RopeSlice};
use sdl2::{event::Event, keyboard::Keycode};

use crate::{
    vim::{Cmd, NewLine},
    vim::{Move, Vim},
    EditorEventResult,
};

#[derive(Copy, Clone)]
pub enum Mode {
    Insert,
    Normal,
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

    vim: Vim,
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
            vim: Vim::new(),
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
            Mode::Normal => self.normal_mode(event),
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
}

// This impl contains utilities for normal mode
impl Editor {
    fn normal_mode(&mut self, event: Event) -> EditorEventResult {
        match self.vim.event(event) {
            None => EditorEventResult::Nothing,
            Some(cmd) => self.handle_cmd(&cmd),
        }
    }

    fn handle_cmd(&mut self, cmd: &Cmd) -> EditorEventResult {
        match cmd {
            Cmd::SwitchMode => {
                self.mode = Mode::Insert;
                EditorEventResult::DrawCursor
            }
            Cmd::Repeat { count, cmd } => {
                if *count == 1 {
                    self.handle_cmd(cmd)
                } else {
                    let mut ret = EditorEventResult::DrawCursor;
                    for _ in 0..*count {
                        ret = self.handle_cmd(cmd);
                    }
                    ret
                }
            }
            Cmd::Delete(None) => {
                self.delete_line(self.line);
                EditorEventResult::DrawText
            }
            Cmd::Delete(Some(mv)) => {
                self.delete_mv(mv);
                EditorEventResult::DrawText
            }
            Cmd::Change(None) => {
                self.mode = Mode::Insert;
                self.delete_line(self.line);
                EditorEventResult::DrawText
            }
            Cmd::Change(Some(mv)) => {
                self.mode = Mode::Insert;
                self.delete_mv(mv);
                EditorEventResult::DrawText
            }
            Cmd::Move(mv) => {
                self.movement(mv);
                EditorEventResult::DrawCursor
            }
            Cmd::NewLine(NewLine { up, switch_mode }) => {
                if *switch_mode {
                    self.mode = Mode::Insert;
                }

                if !up {
                    self.new_line();
                } else {
                    self.new_line_before();
                }

                EditorEventResult::DrawText
            }
            Cmd::SwitchMove(mv) => {
                self.movement(mv);
                self.mode = Mode::Insert;
                EditorEventResult::DrawCursor
            }
            r => todo!("Unimplemented: {:?}", r),
        }
    }

    fn movement(&mut self, mv: &Move) {
        match mv {
            Move::Start => {
                self.cursor = 0;
                self.line = 0;
            }
            Move::End => {
                self.line = self.lines.len() - 1;
                self.cursor = 0;
            }
            Move::Up => self.up(1),
            Move::Down => self.down(1),
            Move::Left => self.left(1),
            Move::Right => self.right(1),
            Move::LineStart => self.move_pos(0),
            Move::LineEnd => self.move_pos(usize::MAX),
            Move::Repeat { count, mv } => {
                for _ in 0..*count {
                    self.movement(mv);
                }
            }
            Move::Find(c) => {
                self.find_line(*c, true);
            }
            Move::ParagraphBegin => {
                self.line = self.prev_paragraph();
                self.sync_line_cursor();
            }
            Move::ParagraphEnd => {
                self.line = self.next_paragraph();
                self.sync_line_cursor();
            }
        }
    }
}

// This impl contains text changing utilities
impl Editor {
    fn delete_mv(&mut self, mv: &Move) {
        let start = self.pos();
        self.movement(mv);
        let end = self.pos();

        match start.cmp(&end) {
            Ordering::Equal => self.delete_range(start..(start + 1)),
            Ordering::Less => self.delete_range(start..end),
            Ordering::Greater => self.delete_range(end..start),
        }
    }

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

    #[inline]
    fn delete_range(&mut self, range: Range<usize>) {
        self.text.remove(range)
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

    fn new_line_before(&mut self) {
        let pos = self.line_pos();
        self.text.insert(if pos == 0 { 0 } else { pos - 1 }, "\n");
        self.cursor = 0;

        self.line = if self.line == 0 { 0 } else { self.line };

        self.lines.insert(self.line, 0);
    }
}

// This impl contains movement utilities
impl Editor {
    /// Return line of the previous paragraph
    #[inline]
    fn prev_paragraph(&mut self) -> usize {
        if self.line == 0 {
            return 0;
        }

        self.lines[0..self.line - 1]
            .iter()
            .enumerate()
            .rev()
            .find(|(_, c)| **c == 0)
            .map_or(0, |(l, _)| l as usize)
    }

    #[inline]
    fn next_paragraph(&mut self) -> usize {
        if self.line == self.lines.len() - 1 {
            return self.line;
        }

        self.lines
            .iter()
            .enumerate()
            .skip(self.line + 1)
            .find(|(_, c)| **c == 0)
            .map_or(self.lines.len() - 1, |(l, _)| l as usize)
    }

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
    fn right(&mut self, count: usize) {
        let c = self.lines[self.line] as usize;
        if self.cursor + count > c {
            self.cursor = if c == 0 { 0 } else { c - 1 };
        } else {
            self.cursor += count;
        }
    }

    fn move_pos(&mut self, pos: usize) {
        if pos > self.lines[self.line] as usize {
            self.cursor = self.lines[self.line] as usize
        } else {
            self.cursor = pos;
        }
    }

    #[inline]
    fn left(&mut self, count: usize) {
        self.cursor -= count;
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
