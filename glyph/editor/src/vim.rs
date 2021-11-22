use sdl2::{event::Event, keyboard::Keycode};

#[derive(Debug, PartialEq)]
pub enum Cmd {
    Repeat { count: u16, cmd: Box<Cmd> },
    Delete(Option<Move>),
    Change(Option<Move>),
    Yank(Option<Move>),
    Move(Move),
    SwitchMove(Move),
    SwitchMode,
    NewLine(NewLine),
}

// 2 d f e

#[derive(Debug, PartialEq)]
pub struct NewLine {
    pub up: bool,
    pub switch_mode: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Move {
    Repeat { count: u16, mv: Box<Move> },
    Left,
    Right,
    Up,
    Down,
    LineStart,
    LineEnd,
    Find(char),
    ParagraphBegin,
    ParagraphEnd,
    Start,
    End,
    Word(bool),
}

#[derive(PartialEq, Debug, Clone)]
pub enum Token {
    Start,
    End,
    Delete,
    Change,
    Yank,
    Find,
    Left,
    Right,
    Up,
    Down,
    LineStart,
    LineEnd,
    ParagraphBegin,
    ParagraphEnd,
    Number(u16),
    Char(char),
    Word(bool),
}

#[derive(Debug, PartialEq)]
enum FailAction {
    Continue,
    // Reset stack
    Reset,
}

type Result<T> = core::result::Result<T, FailAction>;

fn digits_to_num(digits: Vec<u16>) -> u16 {
    let mut num = 0;
    for digit in digits {
        num = num * 10 + digit as u16;
    }
    num
}

pub struct Vim {
    cmd_stack: Vec<Token>,
    parsing_find: bool,
    parsing_start: bool,
    parse_idx: usize,
}

impl Vim {
    pub fn new() -> Self {
        Self {
            cmd_stack: Vec::new(),
            parsing_find: false,
            parsing_start: false,
            parse_idx: 0,
        }
    }

    pub fn event(&mut self, event: Event) -> Option<Cmd> {
        match event {
            Event::KeyDown {
                keycode: Some(key), ..
            } => match key {
                Keycode::H => self.cmd_stack.push(Token::Left),
                Keycode::L => self.cmd_stack.push(Token::Right),
                Keycode::J => self.cmd_stack.push(Token::Down),
                Keycode::K => self.cmd_stack.push(Token::Up),
                Keycode::Num0 | Keycode::Kp0 => {
                    match self.cmd_stack.last().cloned() {
                        Some(Token::Number(n)) => {
                            // self.cmd_stack.push(Token::Number(n * 10));
                        }
                        _ => {} /* self.cmd_stack.push(Token::LineStart) */
                    };
                }
                _ => {}
            },
            Event::TextInput { text, .. } => {
                if self.parsing_start {
                    if text.as_str() == "g" {
                        self.cmd_stack.push(Token::Start);
                        self.parsing_start = false;
                    } else {
                        self.reset();
                    }
                } else if self.parsing_find {
                    self.cmd_stack
                        .push(Token::Char(text.chars().next().unwrap()));
                    self.parsing_find = false;
                } else {
                    match text.as_str() {
                        // Ops
                        "d" => self.cmd_stack.push(Token::Delete),
                        "c" => self.cmd_stack.push(Token::Change),
                        "y" => self.cmd_stack.push(Token::Yank),
                        "f" => {
                            self.cmd_stack.push(Token::Find);
                            self.parsing_find = true
                        }
                        // Movement
                        "g" => {
                            self.parsing_start = true;
                        }
                        "G" => self.cmd_stack.push(Token::End),
                        "A" => return Some(Cmd::SwitchMove(Move::LineEnd)),
                        "a" => return Some(Cmd::SwitchMove(Move::Right)),
                        "O" => {
                            return Some(Cmd::NewLine(NewLine {
                                up: true,
                                switch_mode: true,
                            }))
                        }
                        "o" => {
                            return Some(Cmd::NewLine(NewLine {
                                up: false,
                                switch_mode: true,
                            }))
                        }
                        "i" => return Some(Cmd::SwitchMode),
                        "$" => self.cmd_stack.push(Token::LineEnd),
                        "{" => self.cmd_stack.push(Token::ParagraphBegin),
                        "}" => self.cmd_stack.push(Token::ParagraphEnd),
                        "W" => self.cmd_stack.push(Token::Word(false)),
                        "w" => self.cmd_stack.push(Token::Word(true)),
                        r => {
                            let c = r.chars().next().unwrap();
                            if c.is_numeric() {
                                match self.cmd_stack.last() {
                                    Some(Token::Number(val)) => {
                                        let num = digits_to_num(vec![
                                            *val as u16,
                                            c.to_digit(10).unwrap() as u16,
                                        ]);
                                        self.cmd_stack.pop();
                                        self.cmd_stack.push(Token::Number(num));
                                    }
                                    _ => {
                                        if c == '0' {
                                            self.cmd_stack.push(Token::LineStart);
                                        } else {
                                            self.cmd_stack
                                                .push(Token::Number(c.to_digit(10).unwrap() as u16))
                                        }
                                    }
                                }
                            } else {
                                self.reset();
                            }
                        }
                    }
                }
            }
            _ => {}
        };

        if self.cmd_stack.is_empty() || self.parsing_start {
            return None;
        }

        let result = match self.parse_cmd() {
            Ok(cmd) => {
                self.reset();
                Some(cmd)
            }
            Err(FailAction::Reset) => {
                self.reset();
                None
            }
            Err(FailAction::Continue) => None,
        };

        self.parse_idx = 0;

        result
    }
}

// Parsing
impl Vim {
    fn parse_cmd(&mut self) -> Result<Cmd> {
        match self.next().cloned() {
            None => Err(FailAction::Continue),
            Some(Token::Delete) => self.parse_op(Token::Delete).map(Cmd::Delete),
            Some(Token::Change) => self.parse_op(Token::Change).map(Cmd::Change),
            Some(Token::Yank) => self.parse_op(Token::Yank).map(Cmd::Yank),
            Some(Token::Number(count)) => self.parse_cmd().map(|cmd| Cmd::Repeat {
                count,
                cmd: Box::new(cmd),
            }),
            Some(Token::Find) => {
                if let Some(Token::Char(char)) = self.next() {
                    Ok(Cmd::Move(Move::Find(*char)))
                } else {
                    Err(FailAction::Continue)
                }
            }
            _ => {
                self.back();
                Ok(Cmd::Move(self.parse_move()?))
            }
        }
    }

    fn parse_op(&mut self, kind: Token) -> Result<Option<Move>> {
        match self.next() {
            Some(tok) if tok.eq(&kind) => Ok(None),
            Some(_) => {
                self.back();
                Ok(Some(self.parse_move()?))
            }
            None => Err(FailAction::Continue),
        }
    }

    fn parse_move(&mut self) -> Result<Move> {
        match self.next().cloned() {
            None => Err(FailAction::Continue),
            Some(Token::Up) => Ok(Move::Up),
            Some(Token::Down) => Ok(Move::Down),
            Some(Token::Left) => Ok(Move::Left),
            Some(Token::Right) => Ok(Move::Right),
            Some(Token::LineEnd) => Ok(Move::LineEnd),
            Some(Token::LineStart) => Ok(Move::LineStart),
            Some(Token::ParagraphBegin) => Ok(Move::ParagraphBegin),
            Some(Token::ParagraphEnd) => Ok(Move::ParagraphEnd),
            Some(Token::Start) => Ok(Move::Start),
            Some(Token::End) => Ok(Move::End),
            Some(Token::Word(skip_punctuation)) => Ok(Move::Word(skip_punctuation)),
            Some(Token::Find) => match self.next() {
                Some(Token::Char(char)) => Ok(Move::Find(*char)),
                Some(_) => Err(FailAction::Reset),
                None => Err(FailAction::Continue),
            },
            Some(Token::Number(count)) => self.parse_move().map(|mv| Move::Repeat {
                count,
                mv: Box::new(mv),
            }),
            _ => Err(FailAction::Reset),
        }
    }

    #[inline]
    fn reset(&mut self) {
        self.parsing_start = false;
        self.parsing_find = false;
        self.parse_idx = 0;
        self.cmd_stack.clear();
    }

    #[inline]
    fn next(&mut self) -> Option<&Token> {
        if self.parse_idx >= self.cmd_stack.len() {
            return None;
        }
        let result = &self.cmd_stack[self.parse_idx];
        self.parse_idx += 1;
        Some(result)
    }

    #[inline]
    fn back(&mut self) {
        if self.parse_idx > 0 {
            self.parse_idx -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use sdl2::keyboard::Mod;

    use super::*;

    fn keydown(code: Keycode) -> Event {
        Event::KeyDown {
            timestamp: 0,
            window_id: 0,
            keycode: Some(code),
            scancode: None,
            keymod: Mod::NOMOD,
            repeat: false,
        }
    }

    fn text_input(input: &str) -> Event {
        Event::TextInput {
            timestamp: 0,
            window_id: 0,
            text: input.to_string(),
        }
    }

    fn is_reset(vim: &mut Vim) {
        assert!(!vim.parsing_find);
        assert_eq!(vim.parse_idx, 0);
        assert_eq!(vim.cmd_stack.len(), 0);
    }

    #[cfg(test)]
    mod ops {
        use super::*;

        #[test]
        fn basic_ops() {
            let mut vim = Vim::new();
            let basic = vec![Keycode::H, Keycode::J, Keycode::K, Keycode::L];
            let basic_moves = vec![Move::Left, Move::Down, Move::Up, Move::Right];
            let basic_input = vec!["d", "c", "y"];

            for (i, input) in basic_input.into_iter().enumerate() {
                assert_eq!(vim.event(text_input(input)), None);
                assert_eq!(
                    vim.event(keydown(basic[i])),
                    Some(match input {
                        "d" => Cmd::Delete(Some(basic_moves[i].clone())),
                        "c" => Cmd::Change(Some(basic_moves[i].clone())),
                        "y" => Cmd::Yank(Some(basic_moves[i].clone())),
                        _ => unreachable!(),
                    })
                );
                is_reset(&mut vim);
            }
        }

        #[test]
        fn repeated_ops() {
            let mut vim = Vim::new();
            let counts = vec![3, 4, 2];
            let basic = vec![Keycode::H, Keycode::J, Keycode::K, Keycode::L];
            let basic_moves = vec![Move::Left, Move::Down, Move::Up, Move::Right];
            let basic_input = vec!["d", "c", "y"];

            for (i, input) in basic_input.into_iter().enumerate() {
                assert_eq!(vim.event(text_input(&counts[i].to_string())), None);
                assert_eq!(vim.event(text_input(input)), None);
                let repeated = Cmd::Repeat {
                    count: counts[i],
                    cmd: Box::new(match input {
                        "d" => Cmd::Delete(Some(basic_moves[i].clone())),
                        "c" => Cmd::Change(Some(basic_moves[i].clone())),
                        "y" => Cmd::Yank(Some(basic_moves[i].clone())),
                        _ => unreachable!(),
                    }),
                };
                assert_eq!(vim.event(keydown(basic[i])), Some(repeated));
                is_reset(&mut vim);
            }
        }

        #[test]
        fn complex() {
            let mut vim = Vim::new();
            assert_eq!(vim.event(text_input("2")), None);
            assert_eq!(vim.event(text_input("d")), None);
            assert_eq!(vim.event(text_input("2")), None);
            assert_eq!(vim.event(text_input("f")), None);
            assert_eq!(
                vim.event(text_input("e")),
                Some(Cmd::Repeat {
                    count: 2,
                    cmd: Box::new(Cmd::Delete(Some(Move::Repeat {
                        count: 2,
                        mv: Box::new(Move::Find('e'))
                    })))
                })
            );
        }
    }

    #[cfg(test)]
    mod movement {
        use super::*;

        #[test]
        fn basic_movement() {
            let mut vim = Vim::new();
            assert_eq!(vim.event(keydown(Keycode::H)), Some(Cmd::Move(Move::Left)));
            is_reset(&mut vim);
            assert_eq!(vim.event(keydown(Keycode::K)), Some(Cmd::Move(Move::Up)));
            is_reset(&mut vim);
            assert_eq!(vim.event(keydown(Keycode::J)), Some(Cmd::Move(Move::Down)));
            is_reset(&mut vim);
            assert_eq!(vim.event(keydown(Keycode::L)), Some(Cmd::Move(Move::Right)));
            is_reset(&mut vim);

            assert_eq!(vim.event(text_input("0")), Some(Cmd::Move(Move::LineStart)));
            is_reset(&mut vim);

            assert_eq!(vim.event(text_input("$")), Some(Cmd::Move(Move::LineEnd)));
            is_reset(&mut vim);

            assert_eq!(vim.event(text_input("f")), None);
            assert!(vim.parsing_find);
            assert_eq!(vim.event(text_input(";")), Some(Cmd::Move(Move::Find(';'))));
            is_reset(&mut vim);
        }

        #[test]
        fn repeated_movement() {
            let mut vim = Vim::new();
            assert_eq!(vim.event(text_input("2")), None);
            assert_eq!(
                vim.event(keydown(Keycode::K)),
                Some(Cmd::Repeat {
                    count: 2,
                    cmd: Box::new(Cmd::Move(Move::Up))
                })
            );
            is_reset(&mut vim);

            assert_eq!(vim.event(text_input("2")), None);
            assert_eq!(vim.event(text_input("f")), None);
            assert_eq!(
                vim.event(text_input("k")),
                Some(Cmd::Repeat {
                    count: 2,
                    cmd: Box::new(Cmd::Move(Move::Find('k')))
                })
            );
            is_reset(&mut vim);
        }
    }
}
