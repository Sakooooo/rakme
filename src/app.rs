use std::sync::Arc;

use pixels::{Pixels, SurfaceTexture};
use tiny_skia::{Color, Pixmap};
use winit::{application::ApplicationHandler, event::WindowEvent, window::Window};

pub struct App {
    window: Option<Arc<Window>>,
    pixels: Option<Pixels<'static>>,
}

impl App {
    pub fn new() -> Self {
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

        let mut pixmap = Pixmap::new(width, height).unwrap();
        pixmap.fill(Color::from_rgba8(74, 74, 84, 255));

        let frame = pixels.frame_mut();

        for (dst, px) in frame.chunks_exact_mut(4).zip(pixmap.pixels()) {
            dst[0] = px.red();
            dst[1] = px.green();
            dst[2] = px.blue();
            dst[3] = px.alpha();
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
        _window_id: winit::window::WindowId,
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
