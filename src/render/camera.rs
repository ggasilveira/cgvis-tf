use glam::Vec3;

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
    pub fn lookat(pos: Vec3, target: Vec3, aspect: f32, fovy: f32, znear: f32, zfar: f32) -> Self {
        Self {
            eye: pos,
            target,
            aspect,
            fovy,
            znear,
            zfar,
        }
    }
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
