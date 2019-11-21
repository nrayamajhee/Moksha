use super::scene_tree::build_node;
use std::rc::Rc;
use super::NodeRef;
use crate::{
    dom_factory::{
        add_event, body, document, get_el, get_target_el, get_target_file_result, get_target_files,
        get_target_innerh, get_target_parent_el, icon_btn_w_id, now, query_els, query_html_el,
        set_timeout, get_progress, window,
    },
    log, rc_rcell,
    scene::primitives::create_primitive_node,
    Editor, LightType, Primitive, RcRcell, Viewport,
};
use maud::html;
use std::str::FromStr;
use strum::IntoEnumIterator;
use wasm_bindgen::JsCast;
use web_sys::{EventTarget, FileReader, HtmlInputElement};
pub fn build(editor: &Editor) {
    body()
        .insert_adjacent_html("beforeend", markup().as_str())
        .expect("Couldn't insert console into the DOM!");
    add_events(&editor);
}
fn markup() -> String {
    let markup = html! {
        section #toolbar {
            (icon_btn_w_id("add-mesh", "Add a new object", "add", "A"))
            //(icon_btn_w_id("translate", "Translate selected object", "call_merge", "G"))
            //(icon_btn_w_id("rotate", "Rotate selected object", "360", "R"))
            //(icon_btn_w_id("scale", "Scale selected object", "image_aspect_ratio", "S"))
            (icon_btn_w_id("focus", "Focus view to selected object", "center_focus_weak", "F"))
            (icon_btn_w_id("toggle-perspective", "Switch Perspective", "crop_5_4", "P"))
            (icon_btn_w_id("zoom-in-out", "Zoom in/out view", "zoom_in", "Z"))
        }
        section #mesh-list.panel {
            h3 {"Add Objects" hr{} "Mesh"}
            ul#mesh {
                @for each in Primitive::iter() {
                    li{(each.to_string().as_str())}
                }
            }
            h3 {"File"}
            ul#file {
                li {input multiple="" type="file" id="obj-file" {} label for="obj-file" {"Wavefront OBJ" span.progress{}}}
            }
            h3 {"Light"}
            ul#light {
                @for light in LightType::iter() {
                    li{(light.to_string().as_str())}
                }
            }
        }
    };
    markup.into_string()
}
pub fn handle_persp_toggle(a_view: RcRcell<Viewport>) {
    let icon = query_html_el("#toggle-perspective .material-icons-outlined");
    if icon.inner_html() == "panorama_horizontal" {
        icon.set_inner_html("crop_5_4");
    } else {
        icon.set_inner_html("panorama_horizontal");
    }
    let mut view = a_view.borrow_mut();
    view.switch_projection();
}
fn add_events(editor: &Editor) {
    add_event(
        &document().get_element_by_id("add-mesh").unwrap(),
        "click",
        move |e| {
            get_el("mesh-list").class_list().toggle("shown").unwrap();
        },
    );

    let view = editor.scene().view();
    let a_view = view.clone();
    add_event(
        &document().get_element_by_id("toggle-perspective").unwrap(),
        "click",
        move |_| {
            handle_persp_toggle(a_view.clone());
        },
    );
    let a_view = view.clone();
    let a_active = editor.active_node.clone();
    add_event(
        &document().get_element_by_id("focus").unwrap(),
        "click",
        move |_| {
            if let Some(node) = a_active.borrow().as_ref() {
                a_view.borrow_mut().focus(&node.borrow());
            }
        },
    );
    let a_view = view.clone();
    add_event(
        &document().get_element_by_id("zoom-in-out").unwrap(),
        "mousedown",
        move |_| {
            let mut view = a_view.borrow_mut();
            view.enable_zoom();
        },
    );
    let list = &query_els("#mesh-list #mesh li");
    for i in 0..list.length() {
        let each = list.get(i).unwrap();
        let editor = editor.clone();
        add_event(
            &each.dyn_into::<EventTarget>().unwrap(),
            "click",
            move |e| {
                let scene = &editor.scene;
                let node = rc_rcell(create_primitive_node(
                    scene,
                    Primitive::from_str(&get_target_innerh(&e)).unwrap(),
                ));
                node.borrow().copy_location(&editor.spawn_origin.borrow());
                scene.add(node);
                query_html_el("#scene-tree > ul").remove();
                build_node(
                    &editor,
                    &get_el("scene-tree"),
                    NodeRef::Mutable(scene.root()),
                );
            },
        );
    }
    let scene = editor.scene();
    add_event(&get_el("obj-file"), "input", move |e| {
        let files = get_target_files(&e);
        let (mut obj, mut mtl, mut tex) = (None, None, None);
        for i in 0..files.length() {
            let file = files.item(i as u32).unwrap();
            let file_type = file.type_();
            let file_name = file.name();
            let file_type = if file_type != "" {
                file_type.split("/").next().unwrap().to_string()
            } else {
                let mut n: Vec<&str> = file_name.split(".").collect();
                n.pop().unwrap().to_string()
            };
            if file_type == "obj" {
                obj = Some(file);
            } else if file_type == "mtl" {
                mtl = Some(file);
            } else if file_type == "image" {
                tex = Some(file);
            }
        }
        if let Some(file) = obj {
            let mut total = 1;
            if let Some(_) = &mtl {
                total += 1;
            }
            if let Some(_) = &tex {
                total += 1;
            }
            let reader = FileReader::new().unwrap();
            let scene = scene.clone();
            add_event(&reader, "load", move |e| {
                let obj_src = get_target_file_result(&e);
                if let Some(file) = &mtl {
                    let reader = FileReader::new().unwrap();
                    let scene = scene.clone();
                    add_event(&reader, "load", move |e| {
                        let mtl_src = get_target_file_result(&e);
                        let obj =
                            scene.object_from_obj(None, obj_src.as_str(), Some(mtl_src.as_str()));
                        scene.add(rc_rcell(obj));
                    });
                    reader.read_as_text(file.as_ref());
                } else {
                    log!("Obj file doesn't have material. Will load default material instead.");
                }
            });
            let progress_el = Rc::new(query_html_el("#obj-file + label .progress"));
            progress_el.class_list().remove_1("loaded");
            add_event(&reader, "progress", move |e| {
                let pe = get_progress(e);
                let progress = (pe.loaded() * 100.) / (pe.total() * total as f64);
                log!("Progress" progress);
                progress_el.style().set_property("width", &format!("{}%", progress));
                if progress == 100. {
                    let p = progress_el.clone();
                    set_timeout(move ||{
                        p.class_list().add_1("loaded");
                        p.style().set_property("width", "0");
                    }, 500);
                }
            });
            reader.read_as_text(&file);
        } else {
            log!("You didn't provide obj file! Can't upload anything.");
        }
    });
    let list = &query_els("#mesh-list #light li");
    for i in 0..list.length() {
        let each = list.get(i).unwrap();
        let editor = editor.clone();
        add_event(
            &each.dyn_into::<EventTarget>().unwrap(),
            "click",
            move |e| {
                let scene = &editor.scene;
                let light = scene.light(
                    LightType::from_str(&get_target_innerh(&e)).unwrap(),
                    [1.0, 1.0, 1.0],
                    1.0,
                );
                light
                    .node()
                    .borrow()
                    .copy_location(&editor.spawn_origin.borrow());
                scene.add_light(&light);
                query_html_el("#scene-tree > ul").remove();
                build_node(
                    &editor,
                    &get_el("scene-tree"),
                    NodeRef::Mutable(scene.root()),
                );
            },
        );
    }
}