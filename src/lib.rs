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
