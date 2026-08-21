use winit::{error::EventLoopError, event_loop::EventLoop};

use crate::app::App;

mod app;

fn main() -> Result<(), EventLoopError> {
    let event_loop = EventLoop::new().unwrap();

    event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);

    let mut app = App::new();
    event_loop.run_app(&mut app)
}
