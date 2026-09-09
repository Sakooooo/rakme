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

use crate::editor::{Editor, Key as EditorKey};
use crate::exec;
use crate::font::Font;

pub struct App {
    window: Option<Arc<Window>>,
    pixels: Option<Pixels<'static>>,
    editor: Option<Editor>,
    proxy: EventLoopProxy<exec::Result>,
    files: Vec<String>,
    /// Last known mouse pointer position, in window pixels.
    mouse_pos: (i32, i32),
    ctrl_held: bool,
    /// If set, save one frame here and exit (`RAKME_SCREENSHOT`).
    screenshot_path: Option<PathBuf>,
}

impl App {
    pub fn new(proxy: EventLoopProxy<exec::Result>, files: Vec<String>) -> Self {
        App {
            window: None,
            pixels: None,
            editor: None,
            proxy,
            files,
            mouse_pos: (0, 0),
            ctrl_held: false,
            screenshot_path: std::env::var_os("RAKME_SCREENSHOT").map(PathBuf::from),
        }
    }

    fn draw(&mut self, event_loop: &ActiveEventLoop) {
        let (Some(pixels), Some(editor), Some(window)) = (
            self.pixels.as_mut(),
            self.editor.as_ref(),
            self.window.as_ref(),
        ) else {
            return;
        };
        let size = window.inner_size();
        if size.width == 0 || size.height == 0 {
            return;
        }
        let mut pixmap = Pixmap::new(size.width, size.height).unwrap();
        editor.draw(&mut pixmap);

        if let Some(path) = self.screenshot_path.take() {
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
        for request in editor.pending_commands.drain(..) {
            let proxy = self.proxy.clone();
            exec::spawn(request, move |result| {
                let _ = proxy.send_event(result);
            });
        }
        if let Some((x, y)) = editor.warp.take()
            && let Some(window) = &self.window
        {
            let _ = window.set_cursor_position(PhysicalPosition::new(x, y));
            self.mouse_pos = (x, y);
        }
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn key_event(&mut self, event: &winit::event::KeyEvent) {
        let Some(editor) = self.editor.as_mut() else {
            return;
        };
        let key = match &event.logical_key {
            Key::Named(NamedKey::Backspace) => Some(EditorKey::Backspace),
            Key::Named(NamedKey::Delete) => Some(EditorKey::Delete),
            Key::Named(NamedKey::ArrowLeft) => Some(EditorKey::Left),
            Key::Named(NamedKey::ArrowRight) => Some(EditorKey::Right),
            Key::Named(NamedKey::ArrowUp) => Some(EditorKey::Up),
            Key::Named(NamedKey::ArrowDown) => Some(EditorKey::Down),
            Key::Named(NamedKey::Home) => Some(EditorKey::Home),
            Key::Named(NamedKey::End) => Some(EditorKey::End),
            Key::Named(NamedKey::PageUp) => Some(EditorKey::PageUp),
            Key::Named(NamedKey::PageDown) => Some(EditorKey::PageDown),
            Key::Named(NamedKey::Escape) => Some(EditorKey::Escape),
            Key::Named(NamedKey::Enter) => Some(EditorKey::Char('\n')),
            Key::Named(NamedKey::Tab) => Some(EditorKey::Char('\t')),
            Key::Character(text) if self.ctrl_held => text
                .chars()
                .next()
                .map(|ch| EditorKey::Ctrl(ch.to_ascii_lowercase())),
            _ => None,
        };
        match key {
            Some(key) => editor.key(key),
            None if !self.ctrl_held => {
                if let Some(text) = &event.text {
                    for ch in text.chars() {
                        if !ch.is_control() {
                            editor.key(EditorKey::Char(ch));
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
        let attributes = Window::default_attributes()
            .with_title("rakme")
            .with_inner_size(LogicalSize::new(1024.0, 768.0));
        let window = Arc::new(event_loop.create_window(attributes).unwrap());
        let size = window.inner_size();
        let surface = SurfaceTexture::new(size.width, size.height, window.clone());
        self.pixels = Some(Pixels::new(size.width.max(1), size.height.max(1), surface).unwrap());

        let font_size = std::env::var("RAKME_FONT_SIZE")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(14.0);
        let font = match Font::load(font_size) {
            Ok(font) => font,
            Err(e) => {
                eprintln!("rakme: {e}");
                event_loop.exit();
                return;
            }
        };
        self.editor = Some(Editor::new(
            font,
            size.width as i32,
            size.height as i32,
            &self.files,
        ));
        self.window = Some(window);
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, result: exec::Result) {
        if let Some(editor) = self.editor.as_mut() {
            editor.exec_done(result);
        }
        self.after_event(event_loop);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::Resized(size) => {
                if size.width == 0 || size.height == 0 {
                    return;
                }
                if let Some(pixels) = self.pixels.as_mut() {
                    let _ = pixels.resize_surface(size.width, size.height);
                    let _ = pixels.resize_buffer(size.width, size.height);
                }
                if let Some(editor) = self.editor.as_mut() {
                    editor.resize(size.width as i32, size.height as i32);
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested => self.draw(event_loop),
            WindowEvent::ModifiersChanged(modifiers) => {
                self.ctrl_held = modifiers.state().control_key()
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_pos = (position.x as i32, position.y as i32);
                if let Some(editor) = self.editor.as_mut() {
                    editor.mouse_move(self.mouse_pos.0, self.mouse_pos.1);
                }
                self.after_event(event_loop);
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let button = match button {
                    MouseButton::Left => 1,
                    MouseButton::Middle => 2,
                    MouseButton::Right => 3,
                    _ => return,
                };
                let (x, y) = self.mouse_pos;
                if let Some(editor) = self.editor.as_mut() {
                    match state {
                        ElementState::Pressed => editor.mouse_press(button, x, y),
                        ElementState::Released => editor.mouse_release(button, x, y),
                    }
                }
                self.after_event(event_loop);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => (-y * 3.0).round() as i64,
                    MouseScrollDelta::PixelDelta(pixel_delta) => {
                        let line_height = self
                            .editor
                            .as_ref()
                            .map(|editor| editor.font.line_height)
                            .unwrap_or(16) as f64;
                        (-pixel_delta.y / line_height).round() as i64
                    }
                };
                let (x, y) = self.mouse_pos;
                if let Some(editor) = self.editor.as_mut() {
                    editor.wheel(x, y, lines);
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
