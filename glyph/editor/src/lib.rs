#![feature(option_result_unwrap_unchecked)]

use once_cell::sync::Lazy;

pub use atlas::*;
pub use constants::*;
pub use editor::*;
pub use gl_program::*;
pub use theme::*;
pub use window::*;

mod atlas;
mod constants;
mod editor;
mod gl_program;
mod theme;
mod vim;
mod window;
#[derive(Debug)]

pub enum EventResult {
    Nothing,
    Draw,
    Scroll,
    Quit,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum EditorEvent {
    Nothing,
    DrawText,
    DrawCursor,
    DrawSelection,
    Multiple,
}

pub enum MoveWordKind {
    Next,
    Prev,
    End,
}

pub enum WindowFrameKind {
    Draw,
    Scroll,
}

pub struct MoveWord {
    pub kind: MoveWordKind,
    pub skip_punctuation: bool,
}

pub const ERROR_RED: Color = Color {
    r: 215,
    g: 0,
    b: 21,
    a: 255,
};

pub const HIGHLIGHT_BLUE: Color = Color {
    r: 15,
    g: 191,
    b: 255,
    a: 51,
};

#[repr(C)]
#[derive(Copy, Clone)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub fn floats(&self) -> [f32; 4] {
        [
            self.r as f32 / 255.0,
            self.g as f32 / 255.0,
            self.b as f32 / 255.0,
            self.a as f32 / 255.0,
        ]
    }

    fn from_hex(hex: &str) -> Self {
        let [r, g, b, a] = Color::hex_to_rgba(hex);
        Self { r, g, b, a }
    }

    fn hex_to_rgba(hex: &str) -> [u8; 4] {
        let mut rgba = [0, 0, 0, 255];
        let hex = hex.trim_start_matches('#');
        for (i, c) in hex
            .chars()
            .step_by(2)
            .zip(hex.chars().skip(1).step_by(2))
            .enumerate()
        {
            let c = c.0.to_digit(16).unwrap() << 4 | c.1.to_digit(16).unwrap();
            rgba[i] = c as u8;
        }
        rgba
    }
}

pub type ThemeType = Lazy<Box<dyn Theme + Send + Sync>>;

pub static TOKYO_NIGHT_STORM: Lazy<Box<dyn Theme + Send + Sync>> =
    Lazy::new(|| Box::new(TokyoNightStorm::new()));

pub static GITHUB: Lazy<Box<dyn Theme + Send + Sync>> = Lazy::new(|| Box::new(GithubDark::new()));
