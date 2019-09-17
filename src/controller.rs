use crate::{mesh::Transform, renderer::Renderer};
use nalgebra::{
    Isometry3, Matrix4, Orthographic3, Perspective3, Point3, Unit, UnitQuaternion, Vector3,
};
use std::f32::consts::PI;

#[derive(Copy, Clone)]
pub enum MouseButton {
    LEFT = 0,
    MIDDLE = 1,
    RIGHT = 2,
}

#[derive(Debug, Copy, Clone)]
pub struct ProjectionConfig {
    pub fov: f32,
    pub near: f32,
    pub far: f32,
}

#[derive(Debug, Copy, Clone)]
pub enum Projection {
    Orthographic(Orthographic3<f32>),
    Perspective(Perspective3<f32>),
}

impl Projection {
    fn to_matrix(self) -> Matrix4<f32> {
        match self {
            Projection::Orthographic(proj) => Matrix4::from(proj),
            Projection::Perspective(proj) => Matrix4::from(proj),
        }
    }
    fn unproject_point(&self, point: &Point3<f32>) -> Point3<f32> {
        match self {
            Projection::Orthographic(proj) => proj.unproject_point(&point),
            Projection::Perspective(proj) => proj.unproject_point(&point),
        }
    }
}

#[derive(PartialEq)]
pub enum ProjectionType {
    Orthographic,
    Perspective,
}

fn ortho_from_persp(
    fov: f32,
    aspect_ratio: f32,
    distance: f32,
    clip_len: f32,
) -> Orthographic3<f32> {
    let halfy = fov / 2.;
    let height = distance * halfy.tan();
    let width = height * aspect_ratio;
    Orthographic3::new(-width, width, -height, height, -clip_len, clip_len)
}

/// A dynamic viewport that can switch camera persective as well as targets,
/// look position as well as modes.
pub struct Viewport {
    proj_config: ProjectionConfig,
    initial_config: ProjectionConfig,
    proj: Projection,
    view: Isometry3<f32>,
    initial_view: Isometry3<f32>,
    target: Point3<f32>,
    aspect_ratio: f32,
    speed: f32,
    button: Option<MouseButton>,
    rotate: bool,
    zoom: bool,
}

impl Viewport {
    pub fn new(
        proj_config: ProjectionConfig,
        aspect_ratio: f32,
        proj_type: ProjectionType,
    ) -> Self {
        let pos = Point3::new(0.0, 20.0, 20.0);
        let target = Point3::new(0.0, 0.0, 0.0);
        let up = Vector3::y();
        let view = Isometry3::look_at_rh(&pos, &target, &up);

        let proj = if proj_type == ProjectionType::Perspective {
            Projection::Perspective(Perspective3::new(
                aspect_ratio,
                proj_config.fov,
                proj_config.near,
                proj_config.far,
            ))
        } else {
            Projection::Orthographic(ortho_from_persp(
                proj_config.fov,
                aspect_ratio,
                view.translation.vector.magnitude(),
                proj_config.far,
            ))
        };

        let button = Some(MouseButton::LEFT);
        let rotate = false;
        let zoom = false;

        Self {
            initial_config: proj_config.clone(),
            proj_config,
            proj,
            initial_view: view.clone(),
            view,
            aspect_ratio,
            target,
            speed: 1.0,
            button,
            rotate,
            zoom,
        }
    }
    pub fn view(&self) -> Matrix4<f32> {
        self.view.to_homogeneous()
    }
    pub fn proj(&self) -> Matrix4<f32> {
        self.proj.to_matrix()
    }
    pub fn screen_to_world(&self, point: [f32;3]) -> [f32;3] {
        let p = self.proj.unproject_point(&point.into());
        let p = self.view.inverse().transform_point(&p);
        [p.x,p.y,p.z]
    }
    pub fn screen_to_ray(&self, point: [f32;2]) -> [f32;3] {
        let point = Point3::new(point[0], point[1], -1.);
        let p = self.proj.unproject_point(&point);
        let v = match self.proj {
            Projection::Orthographic(proj) => Vector3::new(p.x,p.y,-p.z / (self.proj_config.near/self.proj_config.far)),
            Projection::Perspective(proj) => Vector3::new(p.x,p.y,p.z),
        };
        let v = self.view.rotation.inverse().transform_vector(&v);
        let v = v.normalize();
        [v.x,v.y,v.z]
    }
    pub fn update_rot(&mut self, dx: i32, dy: i32, dt: f32) {
        if self.rotate {
            let pitch = dy as f32 * 0.01 * self.speed;
            let yaw = dx as f32 * 0.01 * self.speed;
            let delta_rot = {
                let axis = Unit::new_normalize(self.view.rotation.conjugate() * Vector3::x());
                let q_ver = UnitQuaternion::from_axis_angle(&axis, pitch);
                let axis = Unit::new_normalize(Vector3::y());
                let q_hor = UnitQuaternion::from_axis_angle(&axis, yaw);
                q_ver * q_hor
            };
            self.view.rotation *= &delta_rot;
        }
    }
    pub fn update_zoom(&mut self, ds: f32) {
        if self.zoom {
            let delta = 1. + ds;
            self.view.translation.vector = self.speed * delta * self.view.translation.vector;
            self.update_ortho();
        }
    }
    fn create_proj(&self, proj_type: ProjectionType) -> Projection {
        if proj_type == ProjectionType::Perspective {
            Projection::Perspective(Perspective3::new(
                self.aspect_ratio,
                self.proj_config.fov,
                self.proj_config.near,
                self.proj_config.far,
            ))
        } else {
            Projection::Orthographic(ortho_from_persp(
                self.proj_config.fov,
                self.aspect_ratio,
                self.view.translation.vector.magnitude(),
                self.proj_config.far,
            ))
        }
    }
    fn update_ortho(&mut self) {
        if let Projection::Orthographic(_) = self.proj {
            self.proj = self.create_proj(ProjectionType::Orthographic);
        }
    }
    pub fn reset(&mut self) {
        self.view = self.initial_view;
        self.update_ortho();
    }
    pub fn switch_projection(&mut self) {
        self.proj = match self.proj {
            Projection::Perspective(_) => self.create_proj(ProjectionType::Orthographic),
            Projection::Orthographic(_) => self.create_proj(ProjectionType::Perspective),
        };
    }
    pub fn resize(&mut self, aspect_ratio: f32) {
        self.aspect_ratio = aspect_ratio;
        self.proj = match self.proj {
            Projection::Orthographic(_) => self.create_proj(ProjectionType::Orthographic),
            Projection::Perspective(_) => self.create_proj(ProjectionType::Perspective),
        };
    }
    pub fn button(&self) -> Option<MouseButton> {
        self.button
    }
    pub fn projection_type(&self) -> ProjectionType {
        match self.proj {
            Projection::Orthographic(_) => ProjectionType::Orthographic,
            Projection::Perspective(_) => ProjectionType::Perspective,
        }
    }
    pub fn disable_rotation(&mut self) {
        self.rotate = false;
    }
    pub fn enable_rotation(&mut self) {
        self.rotate = true;
    }
    pub fn disable_zoom(&mut self) {
        self.zoom = false;
    }
    pub fn enable_zoom(&mut self) {
        self.zoom = true;
    }
    pub fn get_transform(&self) -> Isometry3<f32> {
        self.view
    }
}
