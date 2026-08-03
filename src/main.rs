use std::{num::NonZeroU32, rc::Rc, sync::Arc};

use pixels::{wgpu::hal::noop::Buffer, Pixels, SurfaceTexture};
use winit::{
    application::ApplicationHandler,
    error::EventLoopError,
    event::WindowEvent,
    event_loop::{EventLoop, OwnedDisplayHandle},
    window::Window,
};

mod window;

struct App {
    window: Option<Arc<Window>>,
    pixels: Option<Pixels<'static>>,
}

const BACKGROUND: [u8; 4] = [28, 28, 28, 255];

impl App {
    fn new() -> Self {
        App {
            window: None,
            pixels: None,
        }
    }

    fn draw(&mut self) {
        let Some(pixels) = self.pixels.as_mut() else {
            return;
        };

        let (width, height) = {
            let size = self.window.as_ref().unwrap().inner_size();
            (size.width, size.height)
        };
        let frame = pixels.frame_mut();

        for y in 0..height {
            for x in 0..width {
                let i = ((y * width + x) * 4) as usize;
                frame[i..i + 4].copy_from_slice(&BACKGROUND);
            }
        }

        pixels.render().unwrap();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes().with_title("rakme"))
                .unwrap(),
        );

        let size = window.inner_size();

        let surface = SurfaceTexture::new(size.width, size.height, window.clone());
        self.pixels = Some(Pixels::new(size.width, size.height, surface).unwrap());

        self.window = Some(window);
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        match event {
            WindowEvent::Resized(size) => {
                if let Some(pixels) = self.pixels.as_mut() {
                    pixels.resize_surface(size.width, size.height).unwrap();
                    pixels.resize_buffer(size.width, size.height).unwrap();
                }
                self.window.as_ref().unwrap().request_redraw();
            }
            WindowEvent::CloseRequested => {
                println!("Exiting...");
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => self.draw(),
            _ => {}
        }
    }
}

fn main() -> Result<(), EventLoopError> {
    let event_loop = EventLoop::new().unwrap();

    event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);

    let mut app = App::new();
    event_loop.run_app(&mut app)
}
