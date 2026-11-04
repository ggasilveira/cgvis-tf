use glam::Vec2;
use std::collections::HashSet;
use winit::event::{KeyEvent, MouseButton, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{KeyCode, PhysicalKey};

#[derive(Debug, Clone)]
pub struct Input {
    pub down_keys: HashSet<KeyCode>,
    pub down_mouse: HashSet<MouseButton>,
    pub clicked_keys: HashSet<KeyCode>,
    pub clicked_mouse: HashSet<MouseButton>,
    pub mouse_delta: Vec2,
}
impl Input {
    pub fn new() -> Self {
        Self {
            down_keys: HashSet::new(),
            down_mouse: HashSet::new(),
            clicked_keys: HashSet::new(),
            clicked_mouse: HashSet::new(),
            mouse_delta: Vec2::ZERO,
        }
    }
    pub fn is_key_down(&self, key: KeyCode) -> bool {
        self.down_keys.contains(&key)
    }
    pub fn is_key_clicked(&self, key: KeyCode) -> bool {
        self.clicked_keys.contains(&key)
    }
    pub fn is_mouse_button_down(&self, button: MouseButton) -> bool {
        self.down_mouse.contains(&button)
    }
    pub fn is_mouse_button_clicked(&self, button: MouseButton) -> bool {
        self.clicked_mouse.contains(&button)
    }
    pub fn mouse_delta(&self) -> Vec2 {
        self.mouse_delta
    }
}
