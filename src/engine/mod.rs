mod api;
mod context;
mod input;
mod level;
mod object;

use crate::math::Transform;
use crate::render::camera::Camera;
use crate::render::light::{DirectionalLight, Light};
use crate::render::manager::RenderManager;
use crate::render::scene::{Instance, Scene};
use std::time::Instant;

use crate::render::asset_manager::AssetManager;
use crate::render::context::RenderContext;
pub use context::EngineContext;
use glam::{Mat4, Quat, Vec2, Vec3};
use input::Input;
use winit::event::{KeyEvent, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::PhysicalKey;

pub struct Engine {
    context: RenderContext,
    input: Input,
    render: RenderManager,
    assets: AssetManager,
    scene: Scene,
    last_frame: Instant,
    last_mouse: Vec2,
    curr_mouse: Vec2,
    cam_ctl: CameraController,
    camera: Camera,
}
impl Engine {
    pub fn new(window: winit::window::Window) -> anyhow::Result<Self> {
        let context = RenderContext::new(window)?;
        let input = Input::new();
        let mut render = RenderManager::new(&context)?;
        let mut assets = AssetManager::new(&context, "assets")?;
        let last_frame = Instant::now();
        let mut scene = Scene::new();
        scene.instances.insert(Instance {
            model: assets.load_glb_model(&context, "models/tree1.glb")?,
            transform: Mat4::from_rotation_translation(Quat::IDENTITY, Vec3::ZERO),
            bodies: Vec::new(),
            mobility: crate::render::scene::Mobility::Static,
            node_overrides: None,
        });
        //render.(&context, &resources, &instances);
        render.scene_render.load_level(&context, &assets, &scene);
        let camera = Camera {
            eye: glam::Vec3::new(0.0, 1.0, 2.0),
            target: glam::Vec3::ZERO,
            aspect: context.config.width as f32 / context.config.height as f32,
            fovy: 45.0,
            znear: 0.1,
            zfar: 100.0,
        };
        render.update_lights(
            &context,
            &[Light::Directional(DirectionalLight {
                direction: Vec3::NEG_Y,
                color: Vec3::new(1., 1., 1.),
                intensity: 1.,
            })],
        );
        Ok(Self {
            context,
            render,
            input,
            assets,
            scene,
            last_frame,
            last_mouse: Vec2::ZERO,
            curr_mouse: Vec2::ZERO,
            cam_ctl: CameraController::new(1.),
            camera,
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
                let now = Instant::now();
                let delta = now - self.last_frame;
                self.input
                    .set_mouse_delta(self.curr_mouse - self.last_mouse);
                self.last_mouse = self.curr_mouse;
                self.update(delta.as_secs_f32());

                match self
                    .render
                    .render(&self.context, &self.assets, &self.scene, self.camera.eye)
                {
                    Ok(_) => {}
                    Err(e) => {
                        log::error!("{e}");
                        event_loop.exit();
                    }
                }
                self.context.window.request_redraw();
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
            WindowEvent::CursorMoved {
                device_id,
                position,
            } => self.curr_mouse = Vec2::new(position.x as f32, position.y as f32),
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
    fn update(&mut self, delta: f32) {
        self.cam_ctl.update_camera(&self.input, &mut self.camera);
        self.render.update_camera(&self.context, &self.camera);
    }
}

pub struct CameraController {
    speed: f32,
}

use winit::keyboard::KeyCode;
impl CameraController {
    pub fn new(speed: f32) -> Self {
        Self { speed }
    }

    pub fn update_camera(&self, input: &Input, camera: &mut crate::render::camera::Camera) {
        use glam::Vec3;
        let forward = camera.target - camera.eye;
        let forward_norm = forward.normalize();
        let forward_mag = forward.length();

        // Prevents glitching when the camera gets too close to the
        // center of the scene.
        if input.is_key_down(KeyCode::KeyW) && forward_mag > self.speed {
            camera.eye += forward_norm * self.speed;
        }
        if input.is_key_down(KeyCode::KeyS) {
            camera.eye -= forward_norm * self.speed;
        }

        let right = forward_norm.cross(Vec3::Y);

        // Redo radius calc in case the forward/backward is pressed.
        let forward = camera.target - camera.eye;
        let forward_mag = forward.length();

        if input.is_key_down(KeyCode::KeyD) {
            // Rescale the distance between the target and the eye so
            // that it doesn't change. The eye, therefore, still
            // lies on the circle made by the target and eye.
            camera.eye = camera.target - (forward + right * self.speed).normalize() * forward_mag;
        }
        if input.is_key_down(KeyCode::KeyA) {
            camera.eye = camera.target - (forward - right * self.speed).normalize() * forward_mag;
        }
    }
}
