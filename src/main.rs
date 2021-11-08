use glyph::{EventResult, Window, SCREEN_HEIGHT, SCREEN_WIDTH};

fn main() {
    let sdl_ctx = sdl2::init().unwrap();
    let video_subsystem = sdl_ctx.video().unwrap();

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

    let mut draw = false;
    'running: loop {
        for event in event_pump.poll_iter() {
            match editor_window.event(event) {
                EventResult::Draw => draw = true,
                EventResult::Quit => break 'running,
                EventResult::Nothing => {}
            }
        }

        if draw {
            window.gl_swap_window();
            draw = false;
        }
    }
}
