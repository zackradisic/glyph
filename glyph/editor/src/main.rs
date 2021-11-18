use core::time;
use std::{
    ffi::CStr,
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

use glyph::{EventResult, Window, SCREEN_HEIGHT, SCREEN_WIDTH, TOKYO_NIGHT_STORM};

fn main() {
    #[cfg(debug_assertions)]
    let filepath_idx = 2;
    #[cfg(not(debug_assertions))]
    let filepath_idx = 1;

    let filepath = std::env::args()
        .nth(filepath_idx)
        .map(|path| fs::read_to_string(path).unwrap());

    let sdl_ctx = sdl2::init().unwrap();
    let video_subsystem = sdl_ctx.video().unwrap();
    let timer = sdl_ctx.timer().unwrap();

    let window = video_subsystem
        .window("glyph", SCREEN_WIDTH, SCREEN_HEIGHT)
        .resizable()
        .allow_highdpi()
        .opengl()
        .build()
        .unwrap();

    let gl_attr = video_subsystem.gl_attr();

    gl_attr.set_context_profile(sdl2::video::GLProfile::Compatibility);
    gl_attr.set_context_version(2, 0);

    let _gl_ctx = window.gl_create_context().unwrap();
    gl::load_with(|s| video_subsystem.gl_get_proc_address(s) as *const std::os::raw::c_void);

    unsafe {
        println!(
            "OpenGL version: {}",
            CStr::from_ptr(gl::GetString(gl::VERSION) as *const i8)
                .to_str()
                .unwrap()
        );
    }

    // Set background
    unsafe {
        gl::Enable(gl::BLEND);
        gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
        gl::Enable(gl::TEXTURE_2D);
        gl::ClearColor(0.0, 0.0, 0.0, 1.0);
        gl::Clear(gl::COLOR_BUFFER_BIT);
    }

    let mut editor_window = Window::new(filepath, &TOKYO_NIGHT_STORM);
    editor_window.render_text();
    window.gl_swap_window();

    let mut event_pump = sdl_ctx.event_pump().unwrap();
    video_subsystem.text_input().start();

    let mut start: u64;
    let mut end: u64;
    let mut elapsed: u64;

    let mut frames: u128 = 0;
    let mut start_now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards!")
        .as_millis();

    'running: loop {
        start = timer.performance_counter();
        unsafe {
            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
            gl::Enable(gl::TEXTURE_2D);
            let bg = editor_window.theme().bg();
            gl::ClearColor(bg.r, bg.g, bg.b, bg.a);
            gl::Clear(gl::COLOR_BUFFER_BIT);
        }

        let mut draw = false;
        for event in event_pump.poll_iter() {
            match editor_window.event(event, timer.ticks()) {
                EventResult::Quit => break 'running,
                EventResult::Draw | EventResult::Nothing => {
                    draw = true;
                }
            }
        }

        frames += 1;
        if draw {
            editor_window.frame(timer.ticks());
            window.gl_swap_window();
        }

        end = timer.performance_counter();
        elapsed = ((end - start) / timer.performance_frequency()) * 1000;

        match SystemTime::now().duration_since(UNIX_EPOCH) {
            Err(_) => {}
            Ok(time) => {
                let ms = time.as_millis();
                if ms - start_now > 5000 {
                    println!(
                        "FPS: {}",
                        frames as f64 / ((time.as_millis() - start_now) as f64 / 1000.0)
                    );
                    frames = 0;
                    start_now = ms;
                }
            }
        }

        std::thread::sleep(time::Duration::from_millis(8/*.666*/ - elapsed));
        // std::thread::sleep(time::Duration::from_millis(1000));
    }
}
