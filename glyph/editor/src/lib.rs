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

pub enum EventResult {
    Nothing,
    Draw,
    Scroll,
    Quit,
}

#[derive(Debug, PartialEq)]
pub enum EditorEvent {
    Nothing,
    DrawText,
    DrawCursor,
}

pub enum MoveWordKind {
    Next,
    Prev,
    End,
}

pub struct MoveWord {
    pub kind: MoveWordKind,
    pub skip_punctuation: bool,
}

#[repr(C)]
#[derive(Clone)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    fn from_hex(hex: &str) -> Self {
        let [r, g, b, a] = Color::hex_to_rgba(hex);
        Self { r, g, b, a }
    }

    fn hex_to_rgba(hex: &str) -> [f32; 4] {
        let mut rgba = [0.0, 0.0, 0.0, 1.0];
        let hex = hex.trim_start_matches('#');
        for (i, c) in hex
            .chars()
            .step_by(2)
            .zip(hex.chars().skip(1).step_by(2))
            .enumerate()
        {
            let c = c.0.to_digit(16).unwrap() << 4 | c.1.to_digit(16).unwrap();
            rgba[i] = c as f32 / 255.0;
        }
        rgba
    }
}

pub type ThemeType = Lazy<Box<dyn Theme + Send + Sync>>;

pub static TOKYO_NIGHT_STORM: Lazy<Box<dyn Theme + Send + Sync>> =
    Lazy::new(|| Box::new(TokyoNightStorm::new()));

pub static GITHUB: Lazy<Box<dyn Theme + Send + Sync>> = Lazy::new(|| Box::new(GithubDark::new()));
