use std::path::PathBuf;
use std::sync::Arc;

use pixels::{Pixels, SurfaceTexture};
use tiny_skia::Pixmap;
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalSize, PhysicalPosition},
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoopProxy},
    keyboard::{Key, NamedKey},
    window::Window,
};

use crate::editor::{Editor, Key as EKey};
use crate::exec;
use crate::font::Font;

pub struct App {
    window: Option<Arc<Window>>,
    pixels: Option<Pixels<'static>>,
    editor: Option<Editor>,
    proxy: EventLoopProxy<exec::Result>,
    files: Vec<String>,
    cursor: (i32, i32),
    ctrl: bool,
    screenshot: Option<PathBuf>,
}

impl App {
    pub fn new(proxy: EventLoopProxy<exec::Result>, files: Vec<String>) -> Self {
        App {
            window: None,
            pixels: None,
            editor: None,
            proxy,
            files,
            cursor: (0, 0),
            ctrl: false,
            screenshot: std::env::var_os("RAKME_SCREENSHOT").map(PathBuf::from),
        }
    }

    fn draw(&mut self, event_loop: &ActiveEventLoop) {
        let (Some(pixels), Some(editor), Some(window)) = (self.pixels.as_mut(), self.editor.as_ref(), self.window.as_ref()) else {
            return;
        };
        let size = window.inner_size();
        if size.width == 0 || size.height == 0 {
            return;
        }
        let mut pixmap = Pixmap::new(size.width, size.height).unwrap();
        editor.draw(&mut pixmap);

        if let Some(path) = self.screenshot.take() {
            if let Err(e) = pixmap.save_png(&path) {
                eprintln!("screenshot: {e}");
            }
            event_loop.exit();
            return;
        }

        let frame = pixels.frame_mut();
        frame.copy_from_slice(pixmap.data());
        if let Err(e) = pixels.render() {
            eprintln!("render: {e}");
        }
    }

    /// Run side effects the editor queued up: shell commands, pointer warps,
    /// quitting.
    fn after_event(&mut self, event_loop: &ActiveEventLoop) {
        let Some(editor) = self.editor.as_mut() else {
            return;
        };
        if editor.quit {
            event_loop.exit();
            return;
        }
        for req in editor.pending.drain(..) {
            let proxy = self.proxy.clone();
            exec::spawn(req, move |r| {
                let _ = proxy.send_event(r);
            });
        }
        if let Some((x, y)) = editor.warp.take()
            && let Some(w) = &self.window
        {
            let _ = w.set_cursor_position(PhysicalPosition::new(x, y));
            self.cursor = (x, y);
        }
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    fn key_event(&mut self, event: &winit::event::KeyEvent) {
        let Some(editor) = self.editor.as_mut() else {
            return;
        };
        let k = match &event.logical_key {
            Key::Named(NamedKey::Backspace) => Some(EKey::Backspace),
            Key::Named(NamedKey::Delete) => Some(EKey::Delete),
            Key::Named(NamedKey::ArrowLeft) => Some(EKey::Left),
            Key::Named(NamedKey::ArrowRight) => Some(EKey::Right),
            Key::Named(NamedKey::ArrowUp) => Some(EKey::Up),
            Key::Named(NamedKey::ArrowDown) => Some(EKey::Down),
            Key::Named(NamedKey::Home) => Some(EKey::Home),
            Key::Named(NamedKey::End) => Some(EKey::End),
            Key::Named(NamedKey::PageUp) => Some(EKey::PageUp),
            Key::Named(NamedKey::PageDown) => Some(EKey::PageDown),
            Key::Named(NamedKey::Escape) => Some(EKey::Escape),
            Key::Named(NamedKey::Enter) => Some(EKey::Char('\n')),
            Key::Named(NamedKey::Tab) => Some(EKey::Char('\t')),
            Key::Character(s) if self.ctrl => s.chars().next().map(|c| EKey::Ctrl(c.to_ascii_lowercase())),
            _ => None,
        };
        match k {
            Some(k) => editor.key(k),
            None if !self.ctrl => {
                if let Some(text) = &event.text {
                    for c in text.chars() {
                        if !c.is_control() {
                            editor.key(EKey::Char(c));
                        }
                    }
                }
            }
            None => {}
        }
    }
}

impl ApplicationHandler<exec::Result> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("rakme")
            .with_inner_size(LogicalSize::new(1024.0, 768.0));
        let window = Arc::new(event_loop.create_window(attrs).unwrap());
        let size = window.inner_size();
        let surface = SurfaceTexture::new(size.width, size.height, window.clone());
        self.pixels = Some(Pixels::new(size.width.max(1), size.height.max(1), surface).unwrap());

        let font_size = std::env::var("RAKME_FONT_SIZE").ok().and_then(|s| s.parse().ok()).unwrap_or(14.0);
        let font = match Font::load(font_size) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("rakme: {e}");
                event_loop.exit();
                return;
            }
        };
        self.editor = Some(Editor::new(font, size.width as i32, size.height as i32, &self.files));
        self.window = Some(window);
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, r: exec::Result) {
        if let Some(ed) = self.editor.as_mut() {
            ed.exec_done(r);
        }
        self.after_event(event_loop);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: winit::window::WindowId, event: WindowEvent) {
        match event {
            WindowEvent::Resized(size) => {
                if size.width == 0 || size.height == 0 {
                    return;
                }
                if let Some(pixels) = self.pixels.as_mut() {
                    let _ = pixels.resize_surface(size.width, size.height);
                    let _ = pixels.resize_buffer(size.width, size.height);
                }
                if let Some(ed) = self.editor.as_mut() {
                    ed.resize(size.width as i32, size.height as i32);
                }
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested => self.draw(event_loop),
            WindowEvent::ModifiersChanged(m) => self.ctrl = m.state().control_key(),
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = (position.x as i32, position.y as i32);
                if let Some(ed) = self.editor.as_mut() {
                    ed.mouse_move(self.cursor.0, self.cursor.1);
                }
                self.after_event(event_loop);
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let btn = match button {
                    MouseButton::Left => 1,
                    MouseButton::Middle => 2,
                    MouseButton::Right => 3,
                    _ => return,
                };
                let (x, y) = self.cursor;
                if let Some(ed) = self.editor.as_mut() {
                    match state {
                        ElementState::Pressed => ed.mouse_press(btn, x, y),
                        ElementState::Released => ed.mouse_release(btn, x, y),
                    }
                }
                self.after_event(event_loop);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => (-y * 3.0).round() as i64,
                    MouseScrollDelta::PixelDelta(p) => {
                        let lh = self.editor.as_ref().map(|e| e.font.line_h).unwrap_or(16) as f64;
                        (-p.y / lh).round() as i64
                    }
                };
                let (x, y) = self.cursor;
                if let Some(ed) = self.editor.as_mut() {
                    ed.wheel(x, y, lines);
                }
                self.after_event(event_loop);
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                self.key_event(&event);
                self.after_event(event_loop);
            }
            _ => {}
        }
    }
}
