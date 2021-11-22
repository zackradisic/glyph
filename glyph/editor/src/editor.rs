use std::{cmp::Ordering, ops::Range};

use ropey::{Rope, RopeSlice};
use sdl2::{event::Event, keyboard::Keycode};

use crate::{
    vim::{Cmd, NewLine},
    vim::{Move, Vim},
    EditorEvent,
};

#[derive(Copy, Clone)]
pub enum Mode {
    Insert,
    Normal,
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

    pub fn event(&mut self, event: Event) -> EditorEvent {
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
                    keycode: Some(Keycode::Tab),
                    ..
                } => {
                    self.insert("  ");
                    EditorEvent::DrawText
                }
                Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => {
                    self.switch_mode(Mode::Normal);
                    EditorEvent::DrawCursor
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
                    EditorEvent::DrawText
                }
                Event::TextInput { text, .. } => {
                    if let Mode::Insert = self.mode {
                        self.insert(&text);
                        EditorEvent::DrawText
                    } else {
                        EditorEvent::Nothing
                    }
                }
                _ => EditorEvent::Nothing,
            },
        }
    }
}

// This impl contains utilities for normal mode
impl Editor {
    fn normal_mode(&mut self, event: Event) -> EditorEvent {
        match self.vim.event(event) {
            None => EditorEvent::Nothing,
            Some(cmd) => self.handle_cmd(&cmd),
        }
    }

    fn handle_cmd(&mut self, cmd: &Cmd) -> EditorEvent {
        match cmd {
            Cmd::SwitchMode => {
                self.switch_mode(Mode::Insert);
                EditorEvent::DrawCursor
            }
            Cmd::Repeat { count, cmd } => {
                let mut ret = EditorEvent::DrawCursor;
                for _ in 0..*count {
                    ret = self.handle_cmd(cmd);
                }
                ret
            }
            Cmd::Delete(None) => {
                self.delete_line(self.line);
                EditorEvent::DrawText
            }
            Cmd::Delete(Some(mv)) => {
                self.delete_mv(mv);
                EditorEvent::DrawText
            }
            Cmd::Change(None) => {
                self.switch_mode(Mode::Insert);
                self.delete_line(self.line);
                EditorEvent::DrawText
            }
            Cmd::Change(Some(mv)) => {
                self.switch_mode(Mode::Insert);
                self.delete_mv(mv);
                EditorEvent::DrawText
            }
            Cmd::Move(mv) => {
                self.movement(mv);
                EditorEvent::DrawCursor
            }
            Cmd::NewLine(NewLine { up, switch_mode }) => {
                if *switch_mode {
                    self.switch_mode(Mode::Insert);
                }

                if !up {
                    self.new_line();
                } else {
                    self.new_line_before();
                }

                EditorEvent::DrawText
            }
            Cmd::SwitchMove(mv) => {
                self.switch_mode(Mode::Insert);
                // Doing `a` at the last char at the end should have same behaviour
                // as doing `A`, meaning we should put cursor under the new-line character (next pos)
                if self.movement(mv) {
                    self.move_pos(usize::MAX);
                }
                EditorEvent::DrawCursor
            }
            r => todo!("Unimplemented: {:?}", r),
        }
    }

    /// Returns true if the movement was truncated (it exceeded the end of the line
    /// and stopped).
    fn movement(&mut self, mv: &Move) -> bool {
        match mv {
            Move::Word(skip_punctuation) => {
                self.next_word(self.line, self.cursor, *skip_punctuation, false)
            }
            Move::Start => {
                self.cursor = 0;
                self.line = 0;
            }
            Move::End => {
                self.line = if self.lines.is_empty() {
                    0
                } else {
                    self.lines.len() - 1
                };
                self.cursor = 0;
            }
            Move::Up => self.up(1),
            Move::Down => self.down(1),
            Move::Left => self.left(1),
            Move::Right => return self.right(1),
            Move::LineStart => self.move_pos(0),
            Move::LineEnd => self.move_pos(usize::MAX),
            Move::Repeat { count, mv } => {
                // TODO: We can be smarter about this and pass
                // the count into the movement, ex. `10l` -> `self.right(10).
                //
                // Additionally, we can stop early for movements like `$` or `0`
                // where repetitions don't affect the cursor anymore.
                for _ in 0..*count {
                    if self.movement(mv) {
                        return true;
                    }
                }
            }
            Move::Find(c) => {
                self.cursor = self.find_line(*c, true).unwrap_or(self.cursor);
            }
            Move::ParagraphBegin => {
                self.line = self.prev_paragraph();
                self.sync_line_cursor();
            }
            Move::ParagraphEnd => {
                self.line = self.next_paragraph();
                self.sync_line_cursor();
            }
        };
        false
    }
}

// This impl contains text changing utilities
impl Editor {
    fn delete_mv(&mut self, mv: &Move) {
        let cursor = self.cursor;
        let line = self.line;
        let start = self.pos();
        let truncated_eol = self.movement(mv);
        let mut end = self.pos();

        if truncated_eol {
            end = self.pos() + 1;
        }

        match start.cmp(&end) {
            Ordering::Equal => self.delete_range(start..(start + 1)),
            Ordering::Less => self.delete_range(start..end),
            Ordering::Greater => self.delete_range(end..start),
        }

        // Return cursor back to starting position
        // TODO: This breaks if we delete backwards for example `d{`
        self.cursor = cursor;
        self.line = line;
    }

    fn insert(&mut self, text: &str) {
        let pos = self.pos();

        self.text.insert(pos, text);
        self.cursor += text.len();
        self.lines[self.line] += text.len() as u32;
    }

    fn backspace(&mut self) -> EditorEvent {
        if self.cursor == 0 && self.line == 0 {
            return EditorEvent::Nothing;
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
        EditorEvent::DrawText
    }

    /// Delete chars in a range.
    ///
    /// If the range spans multiple lines then we just apply it to the entire line,
    /// this is the same behaviour demonstrated by Vim. For example, try the command
    /// `3dj` this will delete the next 3 lines in totality. It doesn't split the lines up.
    #[inline]
    fn delete_range(&mut self, range: Range<usize>) {
        let start_line = self.text.char_to_line(range.start);
        let end_line = self.text.char_to_line(range.end);
        if start_line == end_line {
            self.text.remove(range);
            self.lines[start_line] = self.line_count(start_line) as u32;
        } else {
            let start = self.text.line_to_char(start_line);
            let end = self.text.line_to_char(end_line) + self.text.line(end_line).len_chars();

            self.text.remove(start..end);

            let mut i = start_line;
            for _ in start_line..(end_line + 1) {
                if self.lines.is_empty() {
                    break;
                }
                self.lines.remove(i);
                if i >= self.lines.len() && !self.lines.is_empty() {
                    i -= 1;
                }
            }
        }
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
        match self.lines[self.line] {
            0 => {
                if self.cursor == 0 {
                    self.new_line();
                    return;
                }
            }
            r => {
                if self.cursor == r as usize {
                    self.new_line();
                    return;
                }
            }
        }
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

    fn add_whitespace(&mut self, pos: usize, count: usize) {
        for i in 0..count {
            self.text.insert_char(pos + i, ' ');
        }
    }

    // Insert a new line
    fn new_line(&mut self) {
        let is_last = self.line == self.lines.len() - 1;
        let mut pos =
            self.line_pos() + self.lines[self.line] as usize + if is_last { 0 } else { 1 };
        if is_last {
            self.text.insert(pos, "\n");
            pos += 1;
        }
        let count = self
            .text
            .line(self.line)
            .chars()
            .enumerate()
            .find_map(|(i, c)| if c != ' ' { Some(i) } else { None })
            .unwrap_or(0);
        self.add_whitespace(pos, count);
        if !is_last {
            self.text.insert(pos + count, "\n");
        }

        self.cursor = count;
        self.line += 1;

        if self.line >= self.lines.len() {
            self.lines.push(count as u32)
        } else {
            self.lines.insert(self.line, count as u32);
        }
    }

    fn new_line_before(&mut self) {
        let pos = self.line_pos();
        // The new line character of previous line
        let pos = if pos == 0 { 0 } else { pos };

        let count = self
            .text
            .line(self.line)
            .chars()
            .enumerate()
            .find_map(|(i, c)| if c != ' ' { Some(i) } else { None })
            .unwrap_or(0);

        self.cursor = count;

        self.add_whitespace(pos, count);
        self.text.insert(pos + count, "\n");

        self.line = if self.line == 0 { 0 } else { self.line };

        self.lines.insert(self.line, count as u32);
    }
}

// This impl contains movement utilities
impl Editor {
    #[inline]
    fn next_word(
        &mut self,
        line: usize,
        cursor: usize,
        skip_punctuation: bool,
        match_first_word: bool,
    ) {
        let is_not_last = line < self.lines.len() - 1;
        if self.lines[line] == 0 {
            if is_not_last {
                self.next_word(line + 1, 0, skip_punctuation, true);
            }
            return;
        }

        let chars: Vec<char> = self.text.line(line).chars().collect();
        let len = chars.len();

        // let mut start = cursor;
        let start = self
            .text
            .line(line)
            .chars()
            .enumerate()
            .skip(cursor)
            .find_map(|(i, c)| {
                if Editor::is_word_separator(c, skip_punctuation) {
                    None
                } else {
                    Some(i)
                }
            });

        if start.is_none() {
            if is_not_last {
                return self.next_word(line + 1, 0, skip_punctuation, true);
            }
            return;
        }

        let mut start = unsafe { start.unwrap_unchecked() };

        let mut end = start + 1;
        if end >= len {
            if is_not_last {
                self.next_word(line + 1, 0, skip_punctuation, true);
            }
            return;
        }

        let mut idxs: Vec<(usize, usize)> = Vec::new();
        let mut searching_start = false;

        while end < len && start < len {
            if searching_start {
                if chars[start] == ' ' {
                    start += 1;
                } else {
                    searching_start = false;
                    end = start + 1;
                }
            } else {
                if Editor::is_word_separator(chars[end], skip_punctuation) {
                    idxs.push((start, end));
                    searching_start = true;
                    start = end;
                }
                end += 1;
            }
        }

        if !searching_start {
            idxs.push((start, end));
        }

        match idxs.len() {
            // If no words on line move to first word of nex line
            0 => {
                if is_not_last {
                    self.next_word(line + 1, 0, skip_punctuation, true);
                }
            }
            // If 1 words on line move to first word of next line if there are more lines,
            // otherwise move to last char of word
            1 => {
                let (start, end) = idxs[0];
                if cursor >= start && cursor < end {
                    if is_not_last {
                        self.next_word(line + 1, 0, skip_punctuation, true);
                    } else {
                        self.cursor = end - 1;
                        self.line = line;
                    }
                } else {
                    self.cursor = start;
                    self.line = line;
                }
            }
            _ => {
                let (start, end) = idxs[0];

                if match_first_word {
                    self.cursor = start;
                    self.line = line;
                } else if cursor >= start && cursor < end {
                    self.cursor = idxs[1].0;
                    self.line = line;
                } else {
                    self.cursor = start;
                    self.line = line;
                }
            }
        }
    }

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

    /// Returns true if attempted to move more characters than the line has
    #[inline]
    fn right(&mut self, count: usize) -> bool {
        let c = self.lines[self.line] as usize;
        if self.cursor + count >= c {
            self.cursor = if c == 0 { 0 } else { c - 1 };
            true
        } else {
            self.cursor += count;
            false
        }
    }

    fn move_pos(&mut self, pos: usize) {
        if pos > self.lines[self.line] as usize {
            // Put it on the newline char (the space after the last char of the line),
            // but only on insert mode. This is Vim behaviour
            self.cursor = self.lines[self.line] as usize;
            if matches!(self.mode, Mode::Normal) && self.lines[self.line] > 0 {
                self.cursor -= 1;
            }
        } else {
            self.cursor = pos;
        }
    }

    #[inline]
    fn left(&mut self, count: usize) {
        if count > self.cursor {
            self.cursor = 0;
        } else {
            self.cursor -= count;
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

// This impl contains generic utility functions
impl Editor {
    #[inline]
    fn switch_mode(&mut self, mode: Mode) {
        if let (Mode::Insert, Mode::Normal) = (self.mode, mode) {
            println!("cursor={} count={}", self.cursor, self.lines[self.line]);
            // If we are switching from insert to normal mode and we are on the new-line character,
            // move it back since we disallow that in normal mode
            if self.cursor == self.lines[self.line] as usize && self.cursor > 0 {
                self.cursor -= 1;
            }
        }
        self.mode = mode;
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
    fn text_str(&self) -> Option<&str> {
        self.text_all().as_str()
    }

    #[inline]
    pub fn line(&self) -> usize {
        self.line as usize
    }

    #[inline]
    pub fn lines(&self) -> &[u32] {
        &self.lines
    }

    #[inline]
    pub fn set_line(&mut self, pos: usize) {
        self.line = pos
    }

    #[inline]
    pub fn incr_line(&mut self, pos: i32) {
        self.line += pos as usize;
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

    /// Calculate the amount of chars in the given line (excluding new line characters)
    #[inline]
    fn line_count(&self, idx: usize) -> usize {
        if self.lines.is_empty() {
            0
        } else if idx == self.lines.len() - 1 {
            // If it's the last line then we don't need to subtract the newline character from the count
            self.text.line(idx).len_chars()
        } else {
            // Subtract the new line character from the count
            self.text.line(idx).len_chars() - 1
        }
    }

    #[inline]
    pub fn is_insert(&self) -> bool {
        matches!(self.mode, Mode::Insert)
    }

    fn is_word_separator(c: char, skip_punctuation: bool) -> bool {
        match c {
            ' ' => true,
            '_' => false,
            _ if skip_punctuation => !c.is_alphanumeric(),
            _ => false,
        }
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

        #[cfg(test)]
        mod delete_range {
            use super::*;

            #[test]
            fn single_line() {
                let mut editor = Editor::new();
                editor.insert("1");
                editor.enter();
                editor.insert("1");
                let start = editor.pos();
                editor.insert("2");
                editor.insert("3");
                let end = editor.pos();
                editor.enter();
                editor.insert("1");
                editor.up(1);
                editor.cursor = 0;

                editor.delete_range(start..end);
                assert_eq!(editor.text_str().unwrap(), "1\n1\n1");
                assert_eq!(editor.lines, vec![1, 1, 1]);
            }

            #[test]
            fn single_line_full() {
                let mut editor = Editor::new();
                editor.insert("1");
                editor.enter();
                let start = editor.pos();
                editor.insert("1");
                editor.insert("2");
                editor.insert("3");
                let end = editor.pos();
                editor.enter();
                editor.insert("1");
                editor.up(1);
                editor.cursor = 0;

                editor.delete_range(start..end);
                assert_eq!(editor.text_str().unwrap(), "1\n\n1");
                assert_eq!(editor.lines, vec![1, 0, 1]);
            }

            #[test]
            fn multi_line() {
                let mut editor = Editor::new();
                editor.insert("1");
                editor.enter();
                editor.insert("1");
                editor.insert("2");
                editor.insert("3");
                editor.enter();
                editor.insert("1");
                editor.up(1);
                let start = editor.pos();
                editor.down(1);
                let end = editor.pos();

                editor.delete_range(start..end);
                assert_eq!(editor.text_str().unwrap(), "1\n");
                assert_eq!(editor.lines, vec![1]);
            }

            #[test]
            fn entire_text() {
                let mut editor = Editor::new();
                editor.insert("1");
                editor.insert("2");
                editor.insert("3");
                editor.enter();
                editor.insert("1");
                editor.insert("2");
                editor.insert("3");
                editor.enter();
                editor.insert("1");
                editor.insert("2");
                editor.insert("3");

                // move to start
                editor.cursor = 0;
                editor.line = 0;
                let start = editor.pos();
                editor.line = editor.lines.len() - 1;
                let end = editor.pos();

                editor.delete_range(start..end);
                assert_eq!(editor.text_str().unwrap(), "");
                assert_eq!(editor.lines, vec![]);
            }
        }

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

            assert_eq!(editor.backspace(), EditorEvent::DrawText);
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
