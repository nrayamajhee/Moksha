#![feature(proc_macro_hygiene)]

#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

macro_rules! log {
    ( $( $t:tt )* ) => {
		let document = crate::dom_factory::document();
		let console_el = document.get_element_by_id("console");
		let msg: String = format!( $( $t )* ).into();
		match console_el {
			Some(_) => {
                let para_el = document.query_selector("#console p:first-of-type").unwrap().unwrap();
				web_sys::console::log_1(&wasm_bindgen::JsValue::from_str(&msg));
				para_el.insert_adjacent_html("afterend", &format!("<p>{}</p>", msg)).unwrap();
			},
			None => {
				web_sys::console::log_1(&wasm_bindgen::JsValue::from("Couldn't find console element. Only displaying to the dev console!"));
				web_sys::console::log_1(&wasm_bindgen::JsValue::from_str(&msg));
			}
		}
    }
}

use genmesh::generators::{Cone, Cube, Cylinder, IcoSphere, Plane, SphereUv, Torus};
use maud::html;
use nalgebra::UnitQuaternion;
use std::cell::RefCell;
use std::f32::consts::PI;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{KeyboardEvent, MouseEvent, Performance, WheelEvent, HtmlElement};

pub mod dom_factory;
use dom_factory::{add_event, body, document, request_animation_frame, window};

pub mod controller;
pub mod mesh;
pub mod renderer;
pub mod scene;

use controller::{Viewport, MouseButton};
use mesh::{Geometry, Material};
use scene::{Scene, factory::{ArrowType,create_transform_gizmo}};
use renderer::{DrawMode, Renderer};

pub fn toggle_console(show: bool) {
    let console_el = document().get_element_by_id("console");
    match console_el {
        Some(el) => {
            if show {
                el.class_list().add_1("shown").unwrap();
            } else {
                el.class_list().remove_1("shown").unwrap();
            }
        }
        None => {
            log!("Couldn't find console element!");
        }
    }
}

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    let dom = html! {
        section #console {
            p {"Logs"}
            button#close-console {i.material-icons{"close"}}
        }
        section #toolbar {
            button#open-console {i.material-icons{"assignment"}}
        }
        canvas #gl-canvas oncontextmenu="return false;" {}
    };

    document().set_title("Webshell | Rayamajhee");
    body().set_inner_html(dom.into_string().as_str());

    let mut renderer = Renderer::new(renderer::Config {
        selector: "#gl-canvas",
        pixel_ratio: 1.,
    });

    let cube_geometry = Geometry::from_genmesh(&Cube::new());

    let mut colors = Vec::new();
    let face_colors = vec![
        [1.0, 1.0, 1.0, 1.0], // Front face: white
        [1.0, 0.0, 0.0, 1.0], // Back face: red
        [0.0, 1.0, 0.0, 1.0], // Top face: green
        [0.0, 0.0, 1.0, 1.0], // Bottom face: blue
        [1.0, 1.0, 0.0, 1.0], // Right face: yellow
        [1.0, 0.0, 1.0, 1.0], // Left face: purple
    ];
    for each in face_colors {
        for _ in 0..4 {
            for i in 0..4 {
                colors.push(each[i]);
            }
        }
    }
    let tex_coords = vec![
        // Front
        0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0, // Back
        0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0, // Top
        0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0, // Bottom
        0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0, // Right
        0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0, // Left
        0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0,
    ];

    let cube_tex = Material::from_image_texture(
        renderer.context(),
        "/assets/img/box_tex.png",
        tex_coords,
    )?;

    let scene = Scene::new();

    let cube = scene.object_from_mesh(cube_geometry.clone(), cube_tex);

    let cube2 = scene.object_from_mesh(cube_geometry.clone(), Material::vertex_colors(colors));

    let sphere = scene.object_from_mesh(
        Geometry::from_genmesh(&IcoSphere::subdivide(3)),
        Material::single_color(0.0, 0.0, 1.0, 1.0),
    );

    let cone = scene.object_from_mesh(
        Geometry::from_genmesh(&Cone::new(8)),
        Material::single_color(1.0, 1.0, 0.0, 1.0),
    );

    let cylinder = scene.object_from_mesh(
        Geometry::from_genmesh(&Cylinder::subdivide(8, 2)),
        Material::single_color(1.0, 0.0, 1.0, 1.0),
    );

    let uv_sphere = scene.object_from_mesh(
        Geometry::from_genmesh(&SphereUv::new(8, 16)),
        Material::single_color(0.0, 1.0, 1.0, 1.0),
    );

    let torus = scene.object_from_mesh(
        Geometry::from_genmesh(&Torus::new(2., 0.5, 8, 8)),
        Material::single_color(0.0, 1.0, 1.0, 0.0),
    );

    let grid = scene.object_from_mesh(
        Geometry::from_genmesh_no_normals(&Plane::subdivide(100, 100)),
        Material::single_color_no_shade(1.0, 1.0, 1.0, 1.0),
    );
    grid.set_rotation(UnitQuaternion::from_euler_angles(PI / 2., 0., 0.));
    grid.scale(50.0);

    let translation_gizmo = create_transform_gizmo(&scene, ArrowType::Cone);
    let scale_gizmo = create_transform_gizmo(&scene, ArrowType::Cube);
    let pan_gizmo = create_transform_gizmo(&scene, ArrowType::Sphere);
    translation_gizmo.set_position([-14.0,0.0,0.0]);
    scale_gizmo.set_position([8.0,0.0,0.0]);
    pan_gizmo.set_position([0.0,0.0,0.0]);
    scene.add_with_mode(&grid, DrawMode::Lines);
    scene.add(&translation_gizmo);
    scene.add(&scale_gizmo);
    scene.add(&pan_gizmo);

    cube.set_position([-13.0, 0.0, 0.0]);
    cube2.set_position([13.0, 0.0, 0.0]);
    sphere.set_position([10.0, 10.0, 10.0]);
    sphere.scale(5.0);
    // cone.set_position([0.0, 0.0, -30.0]);
    // torus.set_position([0.0, -5.0, 0.0]);
    // cylinder.set_position([0.0, 5.0, 0.0]);

    // scene.add(&cube);
    // scene.add(&cube2);
    // scene.add(&torus);
    // scene.add(&sphere);
    // scene.add(&uv_sphere);
    // scene.add(&cylinder);
    // scene.add(&cone);

    let viewport = Viewport::new(&renderer);
    renderer.setup_renderer(&scene);
    renderer.update_viewport(&viewport);

    let a_rndr = Rc::new(RefCell::new(renderer));
    let a_scene = Rc::new(RefCell::new(scene));
    let a_view = Rc::new(RefCell::new(viewport));
    let a_cube = Rc::new(RefCell::new(cube));

    let window = window();
    let f = Rc::new(RefCell::new(None));
    let g = f.clone();

    let b_rndr = a_rndr.clone();
    let b_view = a_view.clone();
    *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        let mut renderer = b_rndr.borrow_mut();
        let cube = a_cube.borrow_mut();
        cube.rotate_by(UnitQuaternion::from_euler_angles(0.01, 0.02, 0.));
        renderer.render(&a_scene.borrow());
        renderer.update_viewport(&b_view.borrow());
        request_animation_frame(f.borrow().as_ref().unwrap());
    }) as Box<dyn FnMut()>));

    let b_rndr = a_rndr.clone();
    let b_view = a_view.clone();
    add_event(&window, "resize", move |_| {
        let mut renderer = b_rndr.borrow_mut();
        let mut viewport = b_view.borrow_mut();
        renderer.resize(&mut viewport);
    });

    add_event(
        &document().get_element_by_id("close-console").unwrap(),
        "click",
        move |_| {
            toggle_console(false);
        },
    );

    add_event(
        &document().get_element_by_id("open-console").unwrap(),
        "click",
        move |_| {
            toggle_console(true);
        },
    );

    let b_view = a_view.clone();
    let perf = window.performance().unwrap();
    add_event(&window, "mousemove", move |e| {
        let mut view = b_view.borrow_mut();
        let me = e.dyn_into::<MouseEvent>().unwrap();
        let dt = perf.now();
        view.update_rot(me.movement_x(), me.movement_y(), dt as f32);
    });

    if let Some(button) = a_view.borrow().button() {
        let renderer = a_rndr.borrow();
        let canvas = renderer.canvas();
        let b_view = a_view.clone();
        let b_rndr = a_rndr.clone();
        add_event(canvas, "mousedown", move |e| {
            let mut view = b_view.borrow_mut();
            let renderer = b_rndr.borrow_mut();
            let me = e.dyn_into::<MouseEvent>().unwrap();
            if me.button() == button as i16 {
                renderer.change_cursor(true);
                view.enable_rotation();
            }
        });
        let b_view = a_view.clone();
        let b_rndr = a_rndr.clone();
        add_event(canvas, "mouseup", move |e| {
            let mut view = b_view.borrow_mut();
            let renderer = b_rndr.borrow_mut();
            let me = e.dyn_into::<MouseEvent>().unwrap();
            if me.button() == button as i16 {
                renderer.change_cursor(false);
                view.disable_rotation();
            }
        });
    }

    let b_view = a_view.clone();
    let b_rndr = a_rndr.clone();
    add_event(&b_rndr.borrow().canvas(), "wheel", move |e| {
        let mut view = b_view.borrow_mut();
        let we = e.dyn_into::<WheelEvent>().unwrap();
        let dy = we.delta_y() as f32;
        view.update_zoom(dy);
    });

    let b_view = a_view.clone();
    add_event(&window, "keydown", move |e| {
        let keycode = e.dyn_into::<KeyboardEvent>().unwrap().code();
        if keycode == "Backquote" {
            let console_el = document().get_element_by_id("console");
            match console_el {
                Some(el) => {
                    let shown = el.class_list().contains("shown");
                    toggle_console(!shown);
                }
                None => {
                    log!("Didn't find console element. Not adding event handlers!");
                }
            }
        } else if keycode == "KeyR" {
            let mut view = b_view.borrow_mut();
            view.reset();
        }
    });
    request_animation_frame(g.borrow().as_ref().unwrap());
    Ok(())
}
