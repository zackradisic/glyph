use common::Edit;
use lsp::{Client, LspSender, ServerCapabilities, Url, VersionedTextDocumentIdentifier};
use ropey::{Rope, RopeSlice};
use sdl2::{event::Event, keyboard::Keycode};
use std::{
    cell::Cell,
    cmp::Ordering,
    hint::unreachable_unchecked,
    ops::Range,
    rc::Rc,
    sync::{Arc, RwLock},
};

use crate::{
    vim::{Cmd, NewLine},
    vim::{Move, Vim},
    EditorEvent, MoveWord, MoveWordKind,
};

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Mode {
    Insert,
    Normal,
    Visual,
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

    // Vim stuff
    vim: Vim,
    selection: Option<(u32, u32)>,

    // Undo/redo
    had_space: bool,
    edits: Vec<Edit>,
    redos: Vec<Edit>,
    edit_vecs: Vec<Vec<char>>,

    /// Store EditorEvent::Multiple data here instead of the enum because
    /// it bloats the enum's size: 1 byte -> 16 bytes!!!
    multiple_events_data: [EditorEvent; 3],

    // LSP
    lsp_sender: Option<LspSender>,
    server_capabilities: Option<Rc<ServerCapabilities>>,
    text_doc_id: Option<Arc<RwLock<VersionedTextDocumentIdentifier>>>,
}

fn text_to_lines<I>(text: I) -> Vec<u32>
where
    I: Iterator<Item = char>,
{
    let mut lines = Vec::new();

    let mut count = 0;
    let mut last = 'a';
    for c in text {
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
            Some(text) => (text_to_lines(text.chars()), Rope::from_str(&text)),
            None => (vec![0], Rope::new()),
        };
        Self {
            cursor: 0,
            lines,
            line: 0,
            text,
            mode: Mode::Insert,

            vim: Vim::new(),
            selection: None,
            had_space: false,
            edits: Vec::new(),
            redos: Vec::new(),
            edit_vecs: Vec::new(),

            multiple_events_data: [EditorEvent::Nothing; 3],

            lsp_sender: None,
            server_capabilities: None,
            text_doc_id: None,
        }
    }

    pub fn new() -> Self {
        Editor::with_text(None)
    }

    pub fn configure_lsp(&mut self, lsp_client: &Client) {
        let sender = lsp_client.sender().clone();
        let text_doc_id = VersionedTextDocumentIdentifier {
            uri: Url::parse("file:///Users/zackradisic/Desktop/Code/lsp-test-workspace/src/lib.rs")
                .unwrap(),
            version: 0,
        };
        sender.send_message(Box::new(lsp::text_doc_did_open(
            text_doc_id.uri.clone(),
            "rust".into(),
            text_doc_id.version,
            self.text_all().to_string(),
        )));
        self.server_capabilities = Some(lsp_client.capabilities().clone());
        self.lsp_sender = Some(sender);
        self.text_doc_id = Some(Arc::new(RwLock::new(text_doc_id)))
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
            Mode::Insert => self.insert_mode(event),
            Mode::Visual => self.visual_mode(event),
        }
    }
}

// This impl contains utilities for visual mode
impl Editor {
    /// Visual mode is identical to normal mode except:
    /// * movements adjust the selection start and end
    /// * Change/Delete/Yank don't have any modifiers and instead apply to the selection
    fn visual_mode(&mut self, event: Event) -> EditorEvent {
        match self.vim.event(event) {
            None => EditorEvent::Nothing,
            Some(cmd) => {
                let start = self
                    .selection()
                    .map_or_else(|| self.pos(), |(start, _)| start as usize);
                let result = self.handle_cmd(&cmd);
                let end = self.pos();

                if start == end {
                    self.selection = Some((start as u32, start as u32));
                    self.set_multiple_event_data([
                        EditorEvent::DrawSelection,
                        result,
                        EditorEvent::Nothing,
                    ]);
                    EditorEvent::Multiple
                } else {
                    if let Some(ref mut selection) = self.selection {
                        match start.cmp(&end) {
                            Ordering::Equal => {}
                            Ordering::Less | Ordering::Greater => {
                                selection.1 = end as u32;
                            }
                        }
                    } else if matches!(self.mode, Mode::Visual) {
                        unreachable!("Selection should be set when entering visual mode");
                    }

                    self.set_multiple_event_data([
                        EditorEvent::DrawSelection,
                        result,
                        EditorEvent::Nothing,
                    ]);
                    EditorEvent::Multiple
                }
            }
        }
    }
}

// This impl contains utilities for insert mode
impl Editor {
    fn insert_mode(&mut self, event: Event) -> EditorEvent {
        match event {
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
        match self.mode {
            Mode::Normal => self.handle_cmd_normal(cmd),
            Mode::Visual => self.handle_cmd_visual(cmd),
            _ => panic!("Vim commands should only be executed in normal or visual mode"),
        }
    }

    fn handle_cmd_visual(&mut self, cmd: &Cmd) -> EditorEvent {
        match cmd {
            Cmd::SwitchMode(Mode::Insert) => {
                self.switch_mode(Mode::Insert);
                EditorEvent::Nothing
            }
            Cmd::SwitchMode(Mode::Visual) => {
                self.switch_mode(Mode::Normal);
                EditorEvent::Nothing
            }
            Cmd::Change(None) | Cmd::Delete(None) => {
                self.delete_selection();
                if matches!(cmd, Cmd::Change(None)) {
                    self.switch_mode(Mode::Insert);
                } else {
                    self.switch_mode(Mode::Normal);
                }
                EditorEvent::DrawText
            }
            Cmd::Yank(None) => {
                todo!()
            }
            // Command parser should only return repeated movement commands
            Cmd::Repeat { count, cmd } => self.repeated_cmd(*count, cmd),
            Cmd::Move(mv) => {
                self.movement(mv);
                EditorEvent::DrawCursor
            }
            _ => panic!(
                "Only Delete/Change/Yank/Repetition/Movement commands are valid in visual mode"
            ),
        }
    }

    fn handle_cmd_normal(&mut self, cmd: &Cmd) -> EditorEvent {
        match cmd {
            Cmd::Undo => {
                self.undo();
                EditorEvent::DrawText
            }
            Cmd::Redo => {
                self.redo();
                EditorEvent::DrawText
            }
            Cmd::SwitchMode(mode) => {
                self.switch_mode(*mode);
                EditorEvent::DrawCursor
            }
            Cmd::Repeat { count, cmd } => self.repeated_cmd(*count, cmd),
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

    fn repeated_cmd(&mut self, count: u16, cmd: &Cmd) -> EditorEvent {
        let mut ret = EditorEvent::DrawCursor;
        for _ in 0..count {
            ret = self.handle_cmd(cmd);
        }
        ret
    }

    /// Returns true if the movement was truncated (it exceeded the end of the line
    /// and stopped).
    fn movement(&mut self, mv: &Move) -> bool {
        match mv {
            Move::Word(skip_punctuation) => self.next_word(
                MoveWord {
                    kind: MoveWordKind::Next,
                    skip_punctuation: *skip_punctuation,
                },
                self.line,
                self.cursor,
                false,
            ),
            Move::BeginningWord(skip_punctuation) => self.next_word(
                MoveWord {
                    kind: MoveWordKind::Prev,
                    skip_punctuation: *skip_punctuation,
                },
                self.line,
                self.cursor,
                false,
            ),
            Move::EndWord(skip_punctuation) => self.next_word(
                MoveWord {
                    kind: MoveWordKind::End,
                    skip_punctuation: *skip_punctuation,
                },
                self.line,
                self.cursor,
                false,
            ),
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
            Move::Find(c, reverse) => {
                self.cursor = self.find_line(*c, !reverse).unwrap_or(self.cursor);
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
    fn delete_selection(&mut self) {
        if let Some((start, end)) = self.selection {
            use Ordering::*;

            match start.cmp(&end) {
                Equal | Less => self.delete_range(start as usize..end as usize),
                Greater => self.delete_range(end as usize..start as usize),
            }
        }
    }

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

        let char = text.chars().next().unwrap();
        println!("Insert Pos: {}", pos);
        self.add_edit(Edit::InsertSingle {
            c: char,
            idx: pos as u32,
        });
    }

    fn backspace(&mut self) -> EditorEvent {
        if self.cursor == 0 && self.line == 0 {
            return EditorEvent::Nothing;
        }

        let pos = self.pos();
        let removed: Option<char> = if self.text.len_chars() > 0 {
            let c = self.text.char(if pos == 0 { 0 } else { pos - 1 });
            self.text.remove(pos - 1..pos);
            Some(c)
        } else {
            None
        };
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
        if let Some(c) = removed {
            self.add_edit(Edit::DeleteSingle {
                c,
                idx: pos as u32 - 1,
            });
        }
        EditorEvent::DrawText
    }

    /// Delete chars in a range.
    ///
    /// ### Normal mode
    /// If the range spans multiple lines then we just apply it to the entire line,
    /// this is the same behaviour demonstrated by Vim. For example, try the command
    /// `3dj` this will delete the next 3 lines in totality. It doesn't split the lines up.
    ///
    /// ### Visual mode
    /// Behaves as expected, cutting and splicing lines instead of deleting them in totality
    #[inline]
    fn delete_range(&mut self, range: Range<usize>) {
        let (start, end) = match self.mode {
            // Start and ending lines
            Mode::Normal => (
                self.text.char_to_line(range.start),
                self.text.char_to_line(range.end),
            ),
            Mode::Visual => (range.start, range.end),
            Mode::Insert => panic!("delete_range should not be called in insert mode"),
        };

        if start == end {
            let text: Vec<char> = self.text.slice(range.start..range.end).chars().collect();
            let start = range.start;
            self.text.remove(range);
            self.lines[start] = self.line_count(start) as u32;
            self.edit_vecs.push(text);
            self.add_edit(Edit::Delete {
                start: Cell::new(start as u32),
                str_idx: self.edit_vecs.len() as u32 - 1,
            });
        } else if matches!(self.mode, Mode::Normal) {
            let start = self.text.line_to_char(start);
            let end = self.text.line_to_char(end) + self.text.line(end).len_chars();
            let text: Vec<char> = self.text.slice(start..end).chars().collect();

            self.text.remove(start..end);

            let mut i = start;
            for _ in start..(end + 1) {
                if self.lines.is_empty() {
                    break;
                }
                self.lines.remove(i);
                if i >= self.lines.len() && !self.lines.is_empty() {
                    i -= 1;
                }
            }

            self.edit_vecs.push(text);
            self.add_edit(Edit::Delete {
                start: Cell::new(start as u32),
                str_idx: self.edit_vecs.len() as u32 - 1,
            })
        } else {
            let line_pos = self.text.char_to_line(start);
            let text: Vec<char> = self.text.slice(start..end).chars().collect();

            self.text.remove(start..end);

            self.edit_vecs.push(text);
            self.add_edit(Edit::Delete {
                start: Cell::new(start as u32),
                str_idx: self.edit_vecs.len() as u32 - 1,
            });

            // TODO: Be smarter about this and only compute the lines affected
            self.lines = text_to_lines(self.text.chars());

            self.line = line_pos;
            self.cursor = start - self.text.line_to_char(line_pos);
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
    fn word_indicies(
        &mut self,
        mut start: usize,
        mut end: usize,
        chars: Vec<char>,
        skip_punctuation: bool,
    ) -> Vec<(usize, usize)> {
        let len = chars.len();
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

        idxs
    }

    fn next_word(&mut self, mv: MoveWord, line: usize, mut cursor: usize, match_first_word: bool) {
        use MoveWordKind::*;
        let is_not_last = match mv.kind {
            Next | End => line < (self.lines.len() - 1),
            Prev => line > 0,
        };

        if self.lines[line] == 0 {
            if is_not_last {
                match mv.kind {
                    Next | End => self.next_word(mv, line + 1, 0, true),
                    Prev => self.next_word(mv, line - 1, usize::MAX, true),
                }
            }
            return;
        }

        let chars: Vec<char> = match mv.kind {
            Next | End => self.text.line(line).chars().collect(),
            Prev => {
                let mut chars: Vec<char> = self.text.line(line).chars().collect();
                chars.reverse();
                chars
            }
        };
        let len = chars.len();
        if cursor > len {
            cursor = len - 1;
        }

        let start = self
            .text
            .line(line)
            .chars()
            .enumerate()
            .skip(if matches!(mv.kind, Prev) {
                len - cursor
            } else {
                cursor
            })
            .find_map(|(i, c)| {
                if Editor::is_word_separator(c, mv.skip_punctuation) {
                    None
                } else {
                    Some(i)
                }
            });

        if start.is_none() {
            if is_not_last {
                match mv.kind {
                    Next | End => self.next_word(mv, line + 1, 0, true),
                    Prev => self.next_word(mv, line - 1, usize::MAX, true),
                };
            }
            return;
        }

        let start = unsafe { start.unwrap_unchecked() };

        let end = start + 1;
        if end >= len {
            if is_not_last {
                match mv.kind {
                    Next | End => self.next_word(mv, line + 1, 0, true),
                    Prev => self.next_word(mv, line - 1, usize::MAX, true),
                };
            }
            return;
        }

        let idxs: Vec<(usize, usize)> = {
            let idxs = self.word_indicies(start, end, chars, mv.skip_punctuation);
            if matches!(mv.kind, Prev) {
                idxs.into_iter()
                    .map(|(start, end)| (len - end, len - start))
                    .collect()
            } else {
                idxs
            }
        };

        match idxs.len() {
            // If no words on line move to first word of nex line
            0 => {
                if is_not_last {
                    match mv.kind {
                        Next | End => self.next_word(mv, line + 1, 0, true),
                        Prev => self.next_word(mv, line - 1, usize::MAX, true),
                    }
                }
            }
            // If 1 words on line move to first word of next line if there are more lines,
            // otherwise move to last char of word
            1 => {
                let (start, end) = idxs[0];
                if cursor >= start && cursor < end {
                    if is_not_last {
                        match mv.kind {
                            Next | End => self.next_word(mv, line + 1, 0, true),
                            Prev => self.next_word(mv, line - 1, usize::MAX, true),
                        }
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
                    self.cursor = if matches!(mv.kind, End) {
                        let new = idxs[0].1 - 1;
                        if self.cursor == new {
                            idxs[1].1 - 1
                        } else {
                            new
                        }
                    } else {
                        idxs[1].0
                    };
                    self.line = line;
                } else {
                    self.cursor = if matches!(mv.kind, End) {
                        end - 1
                    } else {
                        start
                    };
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
                .skip(self.cursor + 1)
                .enumerate()
                .find(|(_, c)| *c == char)
                .map(|(pos, _)| self.cursor + pos + 1)
        } else {
            let chars: Vec<char> = self.text.line(self.line).chars().collect();
            for i in (0..self.cursor).rev() {
                if chars[i] == char {
                    return Some(i);
                }
            }
            None
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

// This impl contains functions related to edits/undos/redos
impl Editor {
    fn lsp_edit_text(&self, edit: &Edit) -> String {
        match edit {
            // Inserts just contain the added text
            Edit::InsertSingle { c, .. } => c.to_string(),
            Edit::Insert { str_idx, .. } => self.edit_vecs[*str_idx as usize].iter().collect(),
            // Deletes are empty
            Edit::DeleteSingle { .. } => "".into(),
            Edit::Delete { .. } => "".into(),
        }
    }

    fn cur_pos_to_lsp_pos(&self, pos: usize) -> lsp::Position {
        let line = self.text.char_to_line(pos);
        let character = pos - self.text.line_to_char(line);
        lsp::Position {
            line: line as u32,
            character: character as u32,
        }
    }

    fn to_lsp_edit(&self, edit: &Edit) -> lsp::TextEdit {
        let range = edit.range(&self.edit_vecs);
        lsp::TextEdit {
            range: Some(lsp::Range {
                start: self.cur_pos_to_lsp_pos(range.start as usize),
                end: self.cur_pos_to_lsp_pos(range.end as usize),
            }),
            range_length: None,
            text: self.lsp_edit_text(edit),
        }
    }

    fn add_edit(&mut self, edit: Edit) {
        let mut debounce = true;
        match edit {
            Edit::InsertSingle { c, idx } => self.add_edit_insert_single(c, idx),
            Edit::DeleteSingle { c, idx } => self.add_edit_delete_single(c, idx),
            other => {
                self.edits.push(other);
                debounce = true;
            }
        }
        // Making an edit invalidates the redo stack
        if !self.redos.is_empty() {
            self.redos.clear()
        }
        // Send to LSP
        if let Some(sender) = &self.lsp_sender {
            let text_doc_id = self.text_doc_id.as_ref().unwrap();
            if debounce {
                // Don't increment version here, let the LspSender do it when it
                // finally dispatches edit to server
                sender.send_edit_debounce(
                    self.edits.last().map(|e| self.to_lsp_edit(e)).unwrap(),
                    text_doc_id.clone(),
                );
            } else {
                text_doc_id.write().unwrap().version += 1;
                sender.send_edit(
                    self.edits.last().map(|e| self.to_lsp_edit(e)).unwrap(),
                    text_doc_id.clone(),
                );
            }
        }
    }

    #[inline]
    fn add_edit_delete_single(&mut self, c: char, idx: u32) {
        match self.edits.last_mut() {
            Some(Edit::Delete { start, str_idx }) => {
                let val = start.get();
                if val > 0 {
                    start.set(val - 1)
                }
                self.edit_vecs[*str_idx as usize].push(c);
            }
            None | Some(Edit::Insert { .. }) | Some(Edit::InsertSingle { .. }) => {
                self.edit_vecs.push(vec![c]);
                self.edits.push(Edit::Delete {
                    start: Cell::new((idx as u32).saturating_sub(1)),
                    str_idx: self.edit_vecs.len() as u32 - 1,
                });
            }
            Some(e) => match e {
                Edit::DeleteSingle { c: c2, idx: idx2 } => {
                    self.edit_vecs.push(vec![c, *c2]);
                    *e = Edit::Delete {
                        start: Cell::new(*idx2),
                        str_idx: self.edit_vecs.len() as u32 - 1,
                    }
                }
                // We won't ever reach these
                Edit::InsertSingle { .. } => unsafe { unreachable_unchecked() },
                Edit::Insert { .. } => unsafe { unreachable_unchecked() },
                Edit::Delete { .. } => unsafe { unreachable_unchecked() },
            },
        }
    }

    #[inline]
    fn add_edit_insert_single(&mut self, c: char, idx: u32) {
        match self.edits.last_mut() {
            _ if self.had_space => {
                let vec = vec![c];
                self.edit_vecs.push(vec);
                let str_idx = self.edit_vecs.len() - 1;
                self.edits.push(Edit::Insert {
                    start: Cell::new(idx as u32),
                    str_idx: str_idx as u32,
                });
                self.had_space = false;
            }
            Some(Edit::Insert { str_idx: str, .. }) => {
                let is_space = c == ' ';
                self.edit_vecs[*str as usize].push(c);
                if is_space {
                    self.had_space = true;
                }
            }
            None | Some(Edit::Delete { .. }) | Some(Edit::DeleteSingle { .. }) => {
                self.edit_vecs.push(vec![c]);
                self.edits.push(Edit::Insert {
                    start: Cell::new(idx),
                    str_idx: self.edit_vecs.len() as u32 - 1,
                })
            }
            Some(e) => match e {
                Edit::InsertSingle { c: c2, idx: idx2 } => {
                    self.edit_vecs.push(vec![c, *c2]);
                    *e = Edit::Insert {
                        start: Cell::new(*idx2),
                        str_idx: self.edit_vecs.len() as u32 - 1,
                    }
                }
                // We handle these above
                Edit::Insert { .. } => unsafe { unreachable_unchecked() },
                Edit::Delete { .. } => unsafe { unreachable_unchecked() },
                Edit::DeleteSingle { .. } => unsafe { unreachable_unchecked() },
            },
        }
    }

    #[inline]
    fn undo(&mut self) {
        if let Some(edit) = self.edits.pop() {
            println!("ORIGINAL: {:?} EDIT_VEC: {:?}", edit, self.edit_vecs);
            let inversion = edit.invert();
            self.redos.push(edit);
            println!("INVERSION: {:?}", inversion);
            self.apply_edit(inversion)
        }
    }

    #[inline]
    fn redo(&mut self) {
        if let Some(edit) = self.redos.pop() {
            self.edits.push(edit.clone());
            self.apply_edit(edit);
        }
    }

    #[inline]
    fn apply_edit(&mut self, edit: Edit) {
        match edit {
            Edit::InsertSingle { c, idx } => self.text.insert_char(idx as usize, c),
            Edit::DeleteSingle { c, idx } => self.text.remove((idx as usize)..(idx as usize + 1)),
            Edit::Delete { start, str_idx } => {
                let len = self.edit_vecs[str_idx as usize].len();
                let start = start.get() as usize;
                println!("Start: {} End: {} Len: {}", start, start + len, len);
                self.text.remove(start..(start + len));
            }
            Edit::Insert { start, str_idx } => {
                let str = self.edit_vecs[str_idx as usize].iter().collect::<String>();
                self.text.insert(start.get() as usize, &str);
            }
        };
        // TODO: Be smarter about this and only compute the lines affected
        self.lines = text_to_lines(self.text.chars());
    }
}

// This impl contains generic utility functions
impl Editor {
    #[inline]
    fn switch_mode(&mut self, mode: Mode) {
        match (self.mode, mode) {
            (Mode::Insert, Mode::Normal) => {
                // If we are switching from insert to normal mode and we are on the new-line character,
                // move it back since we disallow that in normal mode
                if self.cursor == self.lines[self.line] as usize && self.cursor > 0 {
                    self.cursor -= 1;
                }
                self.mode = mode;
                self.vim.set_mode(mode);
            }
            (Mode::Normal, Mode::Visual) => {
                let pos = self.pos() as u32;
                self.selection = Some((pos, pos));
                self.mode = mode;
                self.vim.set_mode(mode);
            }
            // Hitting `v` in visual mode should return to normal mode
            (Mode::Visual, Mode::Visual) => {
                self.selection = None;
                self.mode = Mode::Normal;
                self.vim.set_mode(mode);
            }
            // Switching to visual mode only allowed from normal mode
            (_, Mode::Visual) => {}
            (Mode::Visual, _) => {
                self.selection = None;
                self.mode = mode;
                self.vim.set_mode(mode);
            }
            (_, _) => {
                self.mode = mode;
                self.vim.set_mode(mode);
            }
        }
    }

    #[inline]
    pub fn within_selection(&self, i: u32) -> bool {
        if let Some((start, end)) = self.selection {
            match start.cmp(&end) {
                Ordering::Less => i >= start && i <= end,
                Ordering::Greater | Ordering::Equal => i >= end && i <= start,
            }
        } else {
            false
        }
    }

    #[inline]
    pub fn past_selection(&self, i: u32) -> bool {
        if let Some((start, end)) = self.selection {
            match start.cmp(&end) {
                Ordering::Less => i > end,
                Ordering::Greater | Ordering::Equal => i > start,
            }
        } else {
            false
        }
    }

    #[inline]
    pub fn selection(&self) -> Option<(u32, u32)> {
        self.selection
    }

    #[inline]
    pub fn text(&self, range: Range<usize>) -> RopeSlice {
        self.text.slice(range)
    }

    #[inline]
    pub fn text_line_col(&self, range_start: lsp::Position, range_end: lsp::Position) -> RopeSlice {
        // Needs to be at the start of the line because when drawing diagnostics
        // we need to calculate the width from beginning since some chars might
        // have different widths
        let start = self.text.line_to_char(range_start.line as usize);
        let end = self.text.line_to_char(range_end.line as usize) + range_end.character as usize;
        self.text.slice(start..end)
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
            _ if !skip_punctuation => !c.is_alphanumeric(),
            _ => false,
        }
    }

    #[inline]
    pub fn take_multiple_event_data(&mut self) -> [EditorEvent; 3] {
        std::mem::replace(&mut self.multiple_events_data, [EditorEvent::Nothing; 3])
    }

    #[inline]
    fn set_multiple_event_data(&mut self, evts: [EditorEvent; 3]) {
        self.multiple_events_data = evts;
    }

    /// Return the char index of the given line
    #[inline]
    pub fn line_idx(&self, line: usize) -> usize {
        self.text.line_to_char(line)
    }

    #[inline]
    pub fn line_char_idx(&self, line: usize, char: usize) -> usize {
        self.line_idx(line) + char
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
            assert_eq!(vec![0], text_to_lines("".chars()));
        }

        #[test]
        fn single_line() {
            let text = "one line";
            assert_eq!(vec![text.len() as u32], text_to_lines(text.chars()));
        }

        #[test]
        fn multiple_lines() {
            let text = "line 1\nline 2";
            assert_eq!(vec![6, 6], text_to_lines(text.chars()));
        }

        #[test]
        fn trailing_newline() {
            let text = "line 1\n";
            assert_eq!(vec![6, 0], text_to_lines(text.chars()));
        }

        #[test]
        fn leading_newline() {
            let text = "\nline 1\n";
            assert_eq!(vec![0, 6, 0], text_to_lines(text.chars()));
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
                assert_eq!(editor.lines, Vec::<u32>::new());
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
