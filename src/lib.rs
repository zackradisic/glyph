pub use atlas::*;
pub use constants::*;
pub use editor::*;
pub use gl_program::*;
pub use window::*;

mod atlas;
mod constants;
mod editor;
mod gl_program;
mod window;

pub enum EventResult {
    Nothing,
    Draw,
    Quit,
}

#[derive(Debug, PartialEq)]
pub enum EditorEventResult {
    Nothing,
    DrawText,
    DrawCursor,
}

pub enum EditorAction {
    Up(u32),
    Down(u32),
    Delete(Delete),
}

pub enum Delete {
    Line(u32),
}
