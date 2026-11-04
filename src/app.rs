use crate::engine::Engine;
use crate::low::render::RenderState;
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::Window;

pub struct App {
    engine: Option<Engine>,
}
impl App {
    pub fn new() -> Self {
        Self { engine: None }
    }

    pub fn run() -> anyhow::Result<()> {
        env_logger::init();
        let event_loop = EventLoop::with_user_event().build()?;
        let mut app = App::new();
        event_loop.run_app(&mut app)?;
        Ok(())
    }
}

impl ApplicationHandler<()> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // TODO: maybe remove this
        if self.engine.is_some() {
            return;
        }
        let window_attributes = Window::default_attributes();

        let window = event_loop.create_window(window_attributes).unwrap();

        // If we are not on web we can use pollster to
        // await the window creation
        self.engine = Some(Engine::new(window).unwrap());
    }

    // #[allow(unused_mut)]
    // fn user_event(&mut self, _event_loop: &ActiveEventLoop, _: ()) {}
    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let Some(engine) = &mut self.engine else {
            return;
        };
        engine.handle_event(event, event_loop);
    }
}
