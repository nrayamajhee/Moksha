use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Document, Event, EventTarget, HtmlCanvasElement, HtmlElement, Window};

pub fn window() -> Window {
    web_sys::window().expect("No global window found!")
}

pub fn document() -> Document {
    window().document().expect("Window has no document!")
}

pub fn body() -> HtmlElement {
    document().body().expect("Document has no body!")
}

pub fn request_animation_frame(f: &Closure<dyn FnMut()>) {
    window()
        .request_animation_frame(f.as_ref().unchecked_ref())
        .expect("should register `requestAnimationFrame` OK");
}

pub fn get_canvas(selector: &str) -> HtmlCanvasElement {
    let canvas = document().query_selector(selector).unwrap().expect(
        format!(
            "Couldn't find canvas with selector `{}` ! Make sure your DOM has a canvas element",
            selector
        )
        .as_str(),
    );
    let canvas: HtmlCanvasElement = canvas
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .expect("Can't convert the dom element to HtmlCanvasElement!");
    canvas
}

pub fn resize_canvas(canvas: &mut HtmlCanvasElement, pixel_ratio: f64) -> f32 {
    let window = window();
    let pixel_ratio = window.device_pixel_ratio() / pixel_ratio;
    let width: u32 = (window.inner_width().unwrap().as_f64().unwrap() * pixel_ratio) as u32;
    let height: u32 = (window.inner_height().unwrap().as_f64().unwrap() * pixel_ratio) as u32;
    canvas.set_width(width);
    canvas.set_height(height);
    width as f32 / height as f32
}

pub fn add_event<H>(target: &EventTarget, event_type: &str, event_listener: H)
where
    H: 'static + FnMut(Event),
{
    let cl = Closure::wrap(Box::new(event_listener) as Box<dyn FnMut(_)>);
    target
        .add_event_listener_with_callback(event_type, cl.as_ref().unchecked_ref())
        .unwrap();
    cl.forget();
}
