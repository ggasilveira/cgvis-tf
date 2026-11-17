use crate::engine::{Engine, Setup};
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::Window;

pub struct App {
    engine: Option<Engine>,
    setup: Setup,
}
impl App {
    pub fn new(setup: Setup) -> Self {
        Self {
            engine: None,
            setup,
        }
    }

    /// Opens the window and runs the game. `setup` spawns the initial objects.
    pub fn run(setup: Setup) -> anyhow::Result<()> {
        env_logger::init();
        let event_loop = EventLoop::with_user_event().build()?;
        let mut app = App::new(setup);
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
        self.engine = Some(Engine::new(window, self.setup).unwrap());
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
        engine.window_event(event, event_loop);
    }
    fn device_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        let Some(engine) = &mut self.engine else {
            return;
        };
        engine.device_event(device_id, event, event_loop);
    }
}
