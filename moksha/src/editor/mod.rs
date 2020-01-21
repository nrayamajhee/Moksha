pub mod console;
pub mod fps;
mod gizmo;
mod scene_tree;
mod toolbar;
use crate::{
    dom_factory::{add_event, get_el, window},
    mesh::{Geometry, Material},
    object, rc_rcell,
    scene::{
        primitives::{create_origin, create_transform_gizmo, ArrowTip},
        Object, Scene,
    },
    Mesh, Projection, RcRcell, Viewport,
};
use genmesh::generators::Plane;
pub use gizmo::{CollisionConstraint, Gizmo};
use nalgebra::{Point3, UnitQuaternion};
use ncollide3d::query::Ray;
use std::f32::consts::PI;
use std::rc::Rc;
use toolbar::handle_persp_toggle;
use wasm_bindgen::JsCast;
use web_sys::{HtmlCanvasElement, KeyboardEvent, MouseEvent};

/// The main GUI editor that faciliates buttons to manipulate the scene, displays log in a separate
/// window, and displays the scene tree.
#[derive(Clone)]
pub struct Editor {
    scene: Rc<Scene>,
    gizmo: RcRcell<Gizmo>,
    active_node: RcRcell<Option<RcRcell<Object>>>,
    spawn_origin: RcRcell<Object>,
}

pub enum NodeRef<'a> {
    Mutable(RcRcell<Object>),
    Owned(&'a Object),
}

impl Editor {
    pub fn new(scene: Rc<Scene>) -> Self {
        let grid = object!(
            &scene,
            Some(Mesh::new(
                Geometry::from_genmesh_no_normals(&Plane::subdivide(100, 100)),
                Material::new_color_no_shade(0.5, 0.5, 0.5, 1.0),
            )),
            "Grid",
            DrawMode::Lines
        );
        grid.set_scale(50.0);
        grid.set_rotation(UnitQuaternion::from_euler_angles(PI / 2., 0., 0.));
        let gizmo = create_transform_gizmo(&scene, ArrowTip::Cone);
        let spawn_origin = rc_rcell({ create_origin(&scene) });
        scene.add(spawn_origin.clone());
        scene.show(&gizmo);
        let gizmo = Gizmo::new(gizmo);
        let gizmo = rc_rcell(gizmo);
        scene.show(&grid);
        let active_node = rc_rcell(None);
        let mut editor = Self {
            scene: scene.clone(),
            gizmo,
            active_node,
            spawn_origin,
        };
        scene_tree::build(&editor);
        toolbar::build(&editor);
        editor.scale_gizmos();
        editor.add_events();
        editor
    }
    pub fn scale_wrt_eye(&self, object: &Object) -> f32 {
        let view = self.scene().view();
        let view = view.borrow();
        if view.projection_type() == Projection::Perspective {
            let eye = Point3::from(view.eye());
            let o_pos = Point3::from(object.global_position());
            (eye - o_pos).magnitude().into()
        } else {
            view.isometry().translation.vector.magnitude().into()
        }
    }
    pub fn scale_gizmos(&self) {
        let gizmo = self.gizmo.borrow();
        let gizmo = gizmo.object();
        gizmo.set_scale(self.scale_wrt_eye(&gizmo) / 60.);
        let origin = self.spawn_origin.borrow();
        origin.set_scale(self.scale_wrt_eye(&origin) / 60.);
    }
    pub fn set_active_node(&self, object: RcRcell<Object>) {
        self.scene.view().borrow_mut().focus(&object.borrow());
        let gizmo = self.gizmo.borrow();
        gizmo.apply_target_transform(&object.borrow());
        if let Some(object) = self.active_node.borrow().as_ref() {
            object.borrow().set_outline(None);
        }
        object.borrow().set_outline(Some(2.));
        *self.active_node.borrow_mut() = Some(object);
        self.scale_gizmos();
    }
    fn add_events(&mut self) {
        let editor = self.clone();
        let rndr = self.scene.renderer();
        let renderer = rndr.clone();
        add_event(&rndr.borrow().canvas(), "mousedown", move |e| {
            get_el("mesh-list").class_list().remove_1("shown").unwrap();
            let me = e.dyn_into::<MouseEvent>().unwrap();

            let view = editor.scene.view();
            if view.borrow().zooming() {
                return;
            }

            let ray = Self::get_ray_from_screen(&me, &view.borrow(), renderer.borrow().canvas());

            if !editor
                .gizmo
                .borrow_mut()
                .handle_mousedown(&ray, &view.borrow())
            {
                if let Some((object, _)) = editor.scene.root().borrow().collides_w_children(&ray) {
                    editor.set_active_node(object);
                }
            }
        });

        let editor = self.clone();
        let rndr = self.scene.renderer();
        let renderer = rndr.clone();
        add_event(&rndr.borrow().canvas(), "mousemove", move |e| {
            let gizmo = editor.gizmo.borrow();
            let view = editor.scene.view();
            if gizmo.collision_constraint() == CollisionConstraint::None || view.borrow().zooming()
            {
                return;
            }
            let active_node = editor.active_node.borrow();
            {
                let mut view = view.borrow_mut();
                view.disable_rotation();
                let me = e.dyn_into::<MouseEvent>().unwrap();
                let ray = Self::get_ray_from_screen(&me, &view, &renderer.borrow().canvas());
                gizmo.handle_mousemove(&ray, &active_node);
            }
            editor.scale_gizmos();
        });

        let editor = self.clone();
        add_event(&window(), "mousemove", move |e| {
            let view = editor.scene().view();
            {
                let me = e.dyn_into::<MouseEvent>().unwrap();
                view.borrow_mut().update_zoom(me.movement_y() as f64);
            }
            if view.borrow().zooming() {
                editor.scale_gizmos();
            }
        });

        let editor = self.clone();
        let rndr = self.scene.renderer();
        add_event(&rndr.borrow().canvas(), "wheel", move |_| {
            editor.scale_gizmos();
        });

        let a_gizmo = self.gizmo.clone();
        add_event(&rndr.borrow().canvas(), "mouseup", move |_| {
            let mut gizmo = a_gizmo.borrow_mut();
            if gizmo.collision_constraint() == CollisionConstraint::None {
                return;
            }
            gizmo.handle_mouseup();
        });

        let editor = self.clone();
        add_event(&window(), "keydown", move |e| {
            let view = editor.scene().view();
            let keycode = e.dyn_into::<KeyboardEvent>().unwrap().code();
            if keycode == "KeyP" {
                handle_persp_toggle(view.clone())
            } else if keycode == "KeyZ" {
                view.borrow_mut().enable_zoom();
            } else if keycode == "KeyF" {
                if let Some(object) = editor.active_node.borrow().as_ref() {
                    view.borrow_mut().focus(&object.borrow());
                    editor.scale_gizmos();
                }
            } else if keycode == "KeyR" {
                view.borrow_mut().reset();
                editor.scale_gizmos();
            } else if keycode == "KeyA" {
                get_el("mesh-list").class_list().toggle("shown").unwrap();
            }
        });
        let view = self.scene.view();
        add_event(&window(), "keyup", move |e| {
            let keycode = e.dyn_into::<KeyboardEvent>().unwrap().code();
            if keycode == "KeyZ" {
                let mut view = view.borrow_mut();
                view.disable_zoom();
            }
        });
    }
    fn scene(&self) -> Rc<Scene> {
        self.scene.clone()
    }
    fn get_ray_from_screen(
        me: &MouseEvent,
        view: &Viewport,
        canvas: &HtmlCanvasElement,
    ) -> Ray<f32> {
        let (hw, hh) = (
            (canvas.offset_width() / 2) as f32,
            (canvas.offset_height() / 2) as f32,
        );
        let (x, y) = (me.offset_x() as f32 - hw, hh - me.offset_y() as f32);
        let (x, y) = (x / hw, y / hh);
        let ray_pos = view.screen_to_world([x as f32, y as f32, -1.0]);
        let ray_vec = view.screen_to_ray([x as f32, y as f32]);
        Ray::new(ray_pos.into(), ray_vec.into())
    }
}
