use crate::{ Node, Events,
    events::{ViewportEvent, CanvasEvent},
};
use nalgebra::{
    Isometry3, Matrix4, Orthographic3, Perspective3, Point3, Unit, UnitQuaternion, Vector3,
};

/// 3 Button mouse configuration.
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
    pub fn to_matrix(self) -> Matrix4<f32> {
        match self {
            Projection::Orthographic(proj) => Matrix4::from(proj),
            Projection::Perspective(proj) => Matrix4::from(proj),
        }
    }
    pub fn to_matrix_f32(self) -> Matrix4<f32> {
        match self {
            Projection::Orthographic(proj) => Matrix4::from(proj),
            Projection::Perspective(proj) => Matrix4::from(proj),
        }
    }
    pub fn unproject_point(&self, point: &Point3<f32>) -> Point3<f32> {
        match self {
            Projection::Orthographic(proj) => proj.unproject_point(&point),
            Projection::Perspective(proj) => proj.unproject_point(&point),
        }
    }
}

/// Orhtographic or Perspective Projection.
#[derive(PartialEq)]
pub enum ProjectionType {
    Orthographic,
    Perspective,
}

fn ortho_from_persp(
    fov: f32,
    aspect_ratio: f64,
    distance: f32,
    clip_len: f32,
) -> Orthographic3<f32> {
    let halfy = fov / 2.;
    let height = distance * halfy.tan();
    let width = height * aspect_ratio as f32;
    Orthographic3::new(-width, width, -height, height, -clip_len, clip_len)
}

/// A dynamic viewport that can switch camera persective as well as targets,
/// look position, and modes.
pub struct Viewport {
    proj_config: ProjectionConfig,
    proj: Projection,
    view: Isometry3<f32>,
    initial_view: Isometry3<f32>,
    target: Isometry3<f32>,
    aspect_ratio: f64,
    speed: f64,
    button: Option<MouseButton>,
    rotate: bool,
    zoom: bool,
}

impl Viewport {
    pub fn new(proj_config: ProjectionConfig, aspect_ratio: f64) -> Self {
        let view = Isometry3::look_at_rh(&[0., 3., 3.].into(), &[0., 0., 0.].into(), &Vector3::y());

        let proj = Projection::Perspective(Perspective3::new(
            aspect_ratio as f32,
            proj_config.fov,
            proj_config.near,
            proj_config.far,
        ));

        let button = Some(MouseButton::LEFT);
        let target = Isometry3::identity();
        let rotate = false;
        let zoom = false;

        Self {
            proj_config,
            proj,
            initial_view: view,
            view,
            aspect_ratio,
            target,
            speed: 1.0,
            button,
            rotate,
            zoom,
        }
    }
    pub fn update(&mut self, event: &Events, dt: f64) {
        //crate::log!("Viewport changed at"  crate::dom_factory::now());
        match event.canvas {
            CanvasEvent::Grab => {
                self.enable_rotation();
            },
            CanvasEvent::Zoom => {
                self.enable_zoom();
            },
            CanvasEvent::Point => {
                self.disable_zoom();
                self.disable_rotation();
            },
            _=>()
        }
        match event.viewport {
            ViewportEvent::Rotate(dx, dy) => {
                self.update_rot(dx as f64 * dt, dy as f64 * dt);
            }
            ViewportEvent::Zoom(dw) => {
                self.enable_zoom();
                self.update_zoom(dw as f64 * dt);
                self.disable_zoom();
            }
            _=>()
        }
    }
    pub fn view(&self) -> Matrix4<f32> {
        self.view.to_homogeneous() * self.target.inverse().to_homogeneous()
    }
    pub fn proj(&self) -> Matrix4<f32> {
        self.proj.to_matrix()
    }
    pub fn screen_to_world(&self, point: [f32; 3]) -> [f32; 3] {
        let p = self.proj.unproject_point(&point.into());
        let view_m = self.target * self.view.inverse();
        let p = view_m.transform_point(&p);
        [p.x, p.y, p.z]
    }
    pub fn screen_to_ray(&self, point: [f32; 2]) -> [f32; 3] {
        let point = Point3::new(point[0], point[1], -1.);
        let p = self.proj.unproject_point(&point);
        let v = match self.proj {
            Projection::Orthographic(_) => Vector3::new(
                p.x,
                p.y,
                -p.z / (self.proj_config.near / self.proj_config.far),
            ),
            Projection::Perspective(_) => Vector3::new(p.x, p.y, p.z),
        };
        let view_m = self.target * self.view.inverse();
        let v = view_m.transform_vector(&v);
        let v = v.normalize();
        [v.x, v.y, v.z]
    }
    pub fn update_rot(&mut self, dx: f64, dy: f64) {
        if self.rotate {
            let pitch = dy * 0.001 * self.speed;
            let yaw = dx * 0.001 * self.speed;
            let delta_rot = {
                let axis = Unit::new_normalize(self.view.rotation.conjugate() * Vector3::x());
                let q_ver = UnitQuaternion::from_axis_angle(&axis, pitch as f32);
                let axis = Unit::new_normalize(self.target.rotation.conjugate() * Vector3::y());
                let q_hor = UnitQuaternion::from_axis_angle(&axis, yaw as f32);
                q_ver * q_hor
            };
            self.view.rotation *= &delta_rot;
        }
    }
    pub fn update_zoom(&mut self, ds: f64) {
        if self.zoom && ds != 0. {
            let delta = if ds > 0. { 1.05 } else { 0.95 };
            self.view.translation.vector = (self.speed * delta) as f32 * self.view.translation.vector;
            self.update_ortho();
        }
    }
    pub fn reset(&mut self) {
        self.view = self.initial_view;
        self.update_ortho();
    }
    pub fn switch_projection(&mut self) {
        self.proj = match self.proj {
            Projection::Perspective(_) => self.get_proj(ProjectionType::Orthographic),
            Projection::Orthographic(_) => self.get_proj(ProjectionType::Perspective),
        };
    }
    pub fn resize(&mut self, aspect_ratio: f64) {
        self.aspect_ratio = aspect_ratio;
        self.proj = match self.proj {
            Projection::Orthographic(_) => self.get_proj(ProjectionType::Orthographic),
            Projection::Perspective(_) => self.get_proj(ProjectionType::Perspective),
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
    pub fn rotating(&self) -> bool {
        self.rotate
    }
    pub fn disable_rotation(&mut self) {
        self.rotate = false;
    }
    pub fn enable_rotation(&mut self) {
        self.rotate = true;
    }
    pub fn zooming(&self) -> bool {
        self.zoom
    }
    pub fn disable_zoom(&mut self) {
        self.zoom = false;
    }
    pub fn enable_zoom(&mut self) {
        self.zoom = true;
    }
    pub fn transform(&self) -> Isometry3<f32> {
        self.view
    }
    pub fn eye(&self) -> [f32; 3] {
        let v = (self.target * self.view.inverse()).translation.vector;
        [v.x, v.y, v.z]
    }
    pub fn focus(&mut self, node: &Node) {
        self.target = (node.parent_transform() * node.transform()).isometry;
    }
    pub fn get_proj(&self, proj_type: ProjectionType) -> Projection {
        if proj_type == ProjectionType::Perspective {
            Projection::Perspective(Perspective3::new(
                self.aspect_ratio as f32,
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
            self.proj = self.get_proj(ProjectionType::Orthographic);
        }
    }
}
