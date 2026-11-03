mod api;
mod context;
mod input;
mod object;
use crate::low::context::RenderContext;
use crate::low::render::RenderState;
use input::Input;
use winit::event::{KeyEvent, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::PhysicalKey;

pub struct Engine {
    context: RenderContext,
    input: Input,
    render: RenderState,
}
impl Engine {
    pub fn new(window: winit::window::Window) -> anyhow::Result<Self> {
        let context = RenderContext::new(window)?;
        let input = Input::new();
        let render = RenderState::new(&context)?;
        Ok(Self {
            context,
            render,
            input,
        })
    }
    pub fn handle_event(&mut self, event: WindowEvent, event_loop: &ActiveEventLoop) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                self.render
                    .resize(&mut self.context, size.width, size.height)
            }
            WindowEvent::RedrawRequested => {
                engine.update();
                match engine.render() {
                    Ok(_) => {}
                    Err(e) => {
                        log::error!("{e}");
                        event_loop.exit();
                    }
                }
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state,
                        ..
                    },
                ..
            } => {
                if state.is_pressed() {
                    self.input.down_keys.insert(code);
                } else {
                    self.input.down_keys.remove(&code);
                }
            }
            WindowEvent::MouseInput {
                device_id,
                state,
                button,
            } => {
                if state.is_pressed() {
                    self.input.down_mouse.insert(button);
                } else {
                    self.input.down_mouse.remove(&button);
                }
            }
            _ => {}
        }
    }
}
