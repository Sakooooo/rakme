use winit::{error::EventLoopError, event_loop::EventLoop};

use crate::app::App;

mod app;
mod editor;
mod exec;
mod font;
mod frame;
mod gfx;
mod text;

fn main() -> Result<(), EventLoopError> {
    let files: Vec<String> = std::env::args().skip(1).collect();
    let event_loop = EventLoop::<exec::Result>::with_user_event().build()?;
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);

    let mut app = App::new(event_loop.create_proxy(), files);
    event_loop.run_app(&mut app)
}
