#[derive(Debug, Clone)]
pub struct Camera {
    pub eye: glam::Vec3,
    pub target: glam::Vec3,
    pub aspect: f32,
    pub fovy: f32,
    pub znear: f32,
    pub zfar: f32,
}
impl Camera {
    pub fn build_matrix(&self) -> glam::Mat4 {
        use glam::prelude::*;
        let up = Vec3::Y;

        let view = self.target - self.eye;
        let w = (-view).normalize();
        let u = up.cross(w);
        let v = w.cross(u);

        // assuming origin at 0,0,0
        let c = self.eye;

        #[rustfmt::skip]
        let view = Mat4::from_cols(
            u.extend(u.dot(-c)),
            u.extend(v.dot(-c)),
            w.extend(v.dot(-c)),
            Vec4::new(0., 0., 0., 1.)
        );
        // #[rustfmt::skip]
        // let view = Mat4::from_rows_array(
        //     &[
        //         u.x, u.y, u.z, -u.dot(c),
        //         v.x, v.y, v.z, -v.dot(c),
        //         w.x, w.y, w.z, -w.dot(c),
        //         0., 0., 0., 1.
        //     ]
        // );
        let n = self.znear;
        let f = self.zfar;
        let t = n.abs() * (self.fovy / 2.0).tan();
        let b = -t;
        let r = t * self.aspect;
        let l = -r;

        #[rustfmt::skip]
        let ortho = Mat4::from_rows_array(&[
            2.0/(r-l),        0.,        0., -(r+l)/(r-l),
                  0.0, 2.0/(t-b),        0., -(t+b)/(t-b),
                  0.0,       0.0, 2.0/(f-n), -(f+n)/(f-n),
                  0.0,       0.0,       0.0,          1.0
        ]);
        #[rustfmt::skip]
        let proj = Mat4::from_rows_array(&[
             n, 0.,  0.,   0.,
            0.,  n,  0.,   0.,
            0., 0., n+f, -f*n,
            0., 0.,  1.,   0.
        ]);
        //ortho * proj * view
        // TODO: change this to use my matrix
        glam::camera::rh::proj::directx::perspective(self.fovy, self.aspect, self.znear, self.zfar)
            * glam::camera::rh::view::look_at_mat4(self.eye, self.target, up)
    }
}

pub struct CameraController {
    speed: f32,
    is_forward_pressed: bool,
    is_backward_pressed: bool,
    is_left_pressed: bool,
    is_right_pressed: bool,
}

use winit::keyboard::KeyCode;
impl CameraController {
    pub fn new(speed: f32) -> Self {
        Self {
            speed,
            is_forward_pressed: false,
            is_backward_pressed: false,
            is_left_pressed: false,
            is_right_pressed: false,
        }
    }

    pub fn handle_key(&mut self, code: KeyCode, is_pressed: bool) -> bool {
        match code {
            KeyCode::KeyW | KeyCode::ArrowUp => {
                self.is_forward_pressed = is_pressed;
                true
            }
            KeyCode::KeyA | KeyCode::ArrowLeft => {
                self.is_left_pressed = is_pressed;
                true
            }
            KeyCode::KeyS | KeyCode::ArrowDown => {
                self.is_backward_pressed = is_pressed;
                true
            }
            KeyCode::KeyD | KeyCode::ArrowRight => {
                self.is_right_pressed = is_pressed;
                true
            }
            _ => false,
        }
    }

    pub fn update_camera(&self, camera: &mut Camera) {
        use glam::Vec3;
        let forward = camera.target - camera.eye;
        let forward_norm = forward.normalize();
        let forward_mag = forward.length();

        // Prevents glitching when the camera gets too close to the
        // center of the scene.
        if self.is_forward_pressed && forward_mag > self.speed {
            camera.eye += forward_norm * self.speed;
        }
        if self.is_backward_pressed {
            camera.eye -= forward_norm * self.speed;
        }

        let right = forward_norm.cross(Vec3::Y);

        // Redo radius calc in case the forward/backward is pressed.
        let forward = camera.target - camera.eye;
        let forward_mag = forward.length();

        if self.is_right_pressed {
            // Rescale the distance between the target and the eye so
            // that it doesn't change. The eye, therefore, still
            // lies on the circle made by the target and eye.
            camera.eye = camera.target - (forward + right * self.speed).normalize() * forward_mag;
        }
        if self.is_left_pressed {
            camera.eye = camera.target - (forward - right * self.speed).normalize() * forward_mag;
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    view_position: [f32; 4],
    view_proj: [[f32; 4]; 4],
}
impl CameraUniform {
    pub fn new() -> Self {
        Self {
            view_position: [0.; 4],
            view_proj: glam::Mat4::IDENTITY.to_cols_array_2d(),
        }
    }
    pub fn update_view_proj(&mut self, cam: &Camera) {
        // change this to `rows` if the uniform is row major
        self.view_position = cam.eye.extend(1.).to_array();
        self.view_proj = cam.build_matrix().to_cols_array_2d();
    }
}
