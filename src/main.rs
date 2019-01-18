#![feature(duration_float, range_contains, exact_size_is_empty, copy_within)]

extern crate gfx_backend_vulkan as backend;
use ctrlc;
use gfx_hal::PresentMode;
use imgui::ImGui;
use imgui_winit::ImGuiWinit;
use std::net::SocketAddr;
use std::time::Instant;
use structopt::StructOpt;
use winit::{
    ElementState,
    Event,
    EventsLoop,
    VirtualKeyCode,
    Window,
    WindowEvent,
};

pub mod debug;
pub mod double_buffer;
pub mod game;
pub mod graphics;
pub mod logger;
pub mod networking;
pub mod state;
pub mod ui;

#[derive(StructOpt, Debug)]
#[structopt(name = "ball-gfx-hal")]
struct Cli {
    /// Instead of opening a gui window, host a headless server on
    /// this address.
    #[structopt(short = "s", long = "server")]
    host_server: Option<SocketAddr>,
}

fn main() {
    logger::apply().unwrap();

    let cli = Cli::from_args();

    match cli.host_server {
        Some(addr) => {
            let (server, thread) = networking::server::host(addr).unwrap();
            ctrlc::set_handler(move || {
                server.shutdown();
            })
            .unwrap();
            thread.join().unwrap();
        },
        None => run_gui(),
    }
}

fn run_gui() {
    let mut imgui = ImGui::init();
    let mut imgui_winit = ImGuiWinit::new(&mut imgui);
    let mut events_loop = EventsLoop::new();
    let window = Window::new(&events_loop).unwrap();
    let mut window_size = window.get_inner_size().unwrap();

    let mut game_state = state::GameState::default();
    let mut debug = debug::DebugState::default();

    let instance = backend::Instance::create("Ball", 1);
    let surface = instance.create_surface(&window);
    let mut graphics = graphics::Graphics::new(
        &instance,
        surface,
        &mut imgui,
        PresentMode::Immediate,
    );
    let mut circle_rend = graphics::CircleRenderer::new(&mut graphics);

    let mut renderdoc = graphics::renderdoc::init();

    let mut last_frame = Instant::now();

    let mut running = true;
    while running {
        // Wait for vertical blank/etc. before even starting to render.
        graphics.wait_for_frame();

        events_loop.poll_events(|event| {
            imgui_winit.handle_event(&mut imgui, &event);
            if let Event::WindowEvent {
                event,
                ..
            } = event
            {
                game_state.handle_event(&window_size, &event);
                match event {
                    WindowEvent::CloseRequested => {
                        running = false;
                    },
                    WindowEvent::Resized(size) => {
                        window_size = size;
                    },
                    WindowEvent::KeyboardInput {
                        input,
                        ..
                    } => {
                        match input.virtual_keycode {
                            Some(VirtualKeyCode::D)
                                if input.state == ElementState::Pressed =>
                            {
                                debug.show_window = !debug.show_window;
                            }
                            _ => (),
                        }
                    },
                    _ => (),
                }
            }
        });

        let now = Instant::now();
        let frame_time = now.duration_since(last_frame).as_float_secs() as f32;
        last_frame = now;

        game_state.update(frame_time);

        let ui = imgui_winit.frame(&mut imgui, &window);
        debug.ui(&ui, &mut graphics, &mut renderdoc, frame_time);
        game_state.ui(&ui, &debug);

        let _ = graphics.draw_frame(ui, |mut ctx| {
            game_state.draw(now, &mut circle_rend, &mut ctx, &debug);
        });
    }

    // Graphics cleanup.
    circle_rend.destroy(&mut graphics);
    graphics.destroy();
}
