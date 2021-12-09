use std::ptr::null;

use gl::types::{GLint, GLuint, GLvoid};

use crate::constants::MAX_WIDTH;

#[derive(Clone)]
pub struct Glyph {
    pub advance_x: f32,
    pub advance_y: f32,
    pub bitmap_w: f32,
    pub bitmap_h: f32,
    pub bitmap_l: f32,
    pub bitmap_t: f32,
    pub tx: f32, // x offset of glyph in texture coordinates
    pub ty: f32, // y offset of glyph in texture coordinates
}

impl Default for Glyph {
    fn default() -> Self {
        Glyph {
            advance_x: 0.0,
            advance_y: 0.0,
            bitmap_w: 0.0,
            bitmap_h: 0.0,
            bitmap_l: 0.0,
            bitmap_t: 0.0,
            tx: 0.0,
            ty: 0.0,
        }
    }
}

pub struct Atlas {
    pub tex: GLuint,
    pub w: u32,
    pub h: u32,
    pub max_h: f32,
    pub max_w: f32,
    pub glyphs: Vec<Glyph>,
}

const CHAR_END: usize = 128;

impl Atlas {
    pub fn new(font_path: &str, height: u32, uniform_tex: GLint) -> Result<Self, String> {
        let ft_lib = freetype::Library::init().unwrap();
        let face = ft_lib.new_face(font_path, 0).unwrap();
        let mut tex: GLuint = 0;

        face.set_pixel_sizes(0, height).map_err(|e| e.to_string())?;

        let g = face.glyph();

        let mut glyphs: Vec<Glyph> = vec![Default::default(); CHAR_END];

        let mut roww: u32 = 0;
        let mut rowh: u32 = 0;
        let mut w: u32 = 0;
        let mut h: u32 = 0;

        let mut max_w = 0u32;

        // Find minimum size for a texture holding all visible ASCII characters
        for i in 32..CHAR_END {
            face.load_char(i, freetype::face::LoadFlag::RENDER)
                .map_err(|e| e.to_string())?;

            if roww + g.bitmap().width() as u32 + 1 >= MAX_WIDTH {
                w = std::cmp::max(w, roww);
                h += rowh;
                roww = 0;
                rowh = 0;
            }

            max_w = std::cmp::max(max_w, g.bitmap().width() as u32);

            roww += g.bitmap().width() as u32 + 1;
            rowh = std::cmp::max(rowh, g.bitmap().rows() as u32);
        }
        let max_h: f32 = rowh as f32;

        w = std::cmp::max(w, roww);
        h += rowh;

        unsafe {
            // Create a texture that will be used to hold all ASCII glyphs
            gl::ActiveTexture(gl::TEXTURE0);
            gl::GenTextures(1, &mut tex as *mut GLuint);
            gl::BindTexture(gl::TEXTURE_2D, tex);
            gl::Uniform1i(uniform_tex, 0);

            gl::TexImage2D(
                gl::TEXTURE_2D,
                0,
                gl::ALPHA as i32,
                w as i32,
                h as i32,
                0,
                gl::ALPHA,
                // gl::RED,
                gl::UNSIGNED_BYTE,
                null(),
            );

            // We require 1 byte alignment when uploading texture data
            gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);

            // Clamping to edges is important to prevent artifacts when scaling
            gl::TexParameteri(
                gl::TEXTURE_2D,
                gl::TEXTURE_WRAP_S,
                gl::CLAMP_TO_EDGE as GLint,
            );
            gl::TexParameteri(
                gl::TEXTURE_2D,
                gl::TEXTURE_WRAP_T,
                gl::CLAMP_TO_EDGE as GLint,
            );

            // Linear filtering usually looks best for text
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as GLint);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as GLint);
        }

        // Paste all glyph bitmaps into the texture, remembering the offset
        let mut ox: i32 = 0;
        let mut oy: i32 = 0;

        rowh = 0;

        for i in 32..CHAR_END {
            face.load_char(i, freetype::face::LoadFlag::RENDER)
                .map_err(|e| e.to_string())?;

            if ox + g.bitmap().width() + 1 >= MAX_WIDTH as i32 {
                ox = 0;
                oy += rowh as i32;
                rowh = 0;
            }

            unsafe {
                gl::TexSubImage2D(
                    gl::TEXTURE_2D,
                    0,
                    ox as i32,
                    oy as i32,
                    g.bitmap().width() as i32,
                    g.bitmap().rows() as i32,
                    gl::ALPHA,
                    gl::UNSIGNED_BYTE,
                    g.bitmap().buffer().as_ptr() as *const GLvoid,
                );
            }

            glyphs[i] = Glyph {
                bitmap_w: g.bitmap().width() as f32,
                bitmap_h: g.bitmap().rows() as f32,
                bitmap_l: g.bitmap_left() as f32,
                bitmap_t: g.bitmap_top() as f32,
                tx: ox as f32 / w as f32,
                ty: oy as f32 / h as f32,
                // 1 unit = 1/64 pixels so bitshift
                // by 6 to get advance in pixels
                advance_x: (g.advance().x >> 6) as f32,
                advance_y: (g.advance().y >> 6) as f32,
            };

            rowh = std::cmp::max(rowh, g.bitmap().rows() as u32);
            ox += g.bitmap().width() + 1;
        }

        // println!(
        //     "Generated a {} x {} ({} kb) texture atlas\n",
        //     w,
        //     h,
        //     w * h / 1024
        // );

        Ok(Self {
            tex,
            w,
            h,
            glyphs,
            max_h,
            max_w: max_w as f32,
        })
    }
}

impl Drop for Atlas {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteTextures(1, &self.tex);
        }
    }
}
