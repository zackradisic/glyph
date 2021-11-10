use core::time;

use glyph::{EventResult, Window, SCREEN_HEIGHT, SCREEN_WIDTH};

fn main() {
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

    // Set background
    unsafe {
        gl::Enable(gl::BLEND);
        gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
        gl::Enable(gl::TEXTURE_2D);
        gl::ClearColor(0.157, 0.157, 0.157, 1.0);
        gl::Clear(gl::COLOR_BUFFER_BIT);
    }

    let mut editor_window = Window::new();
    editor_window.queue_cursor();
    window.gl_swap_window();

    let mut event_pump = sdl_ctx.event_pump().unwrap();
    video_subsystem.text_input().start();

    let mut start: u64;
    let mut end: u64;
    let mut elapsed: u64;

    'running: loop {
        start = timer.performance_counter();
        unsafe {
            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
            gl::Enable(gl::TEXTURE_2D);
            gl::ClearColor(0.157, 0.157, 0.157, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
        }

        for event in event_pump.poll_iter() {
            match editor_window.event(event) {
                EventResult::Quit => break 'running,
                EventResult::Draw | EventResult::Nothing => {}
            }
        }

        editor_window.frame();
        window.gl_swap_window();

        end = timer.performance_counter();
        elapsed = ((end - start) / timer.performance_frequency()) * 1000;

        std::thread::sleep(time::Duration::from_millis(16/*.666*/ - elapsed));
        // std::thread::sleep(time::Duration::from_millis(1000));
    }
}
