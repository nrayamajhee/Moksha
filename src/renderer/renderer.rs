use super::shader::{
    bind_attribute, bind_buffer_and_attribute, bind_buffer_f32, bind_index_buffer,
    bind_uniform_i32, bind_uniform_mat4, bind_uniform_vec4, create_color_program,
    create_texture_program, create_vertex_color_program, ShaderType,
};
use crate::dom_factory::{get_canvas, resize_canvas};
use crate::{controller::Viewport, mesh::Storage};
use cgmath::{prelude::*, Matrix4, SquareMatrix};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use strum::IntoEnumIterator;
use wasm_bindgen::{prelude::*, JsCast, JsValue};
use web_sys::{
    HtmlCanvasElement, WebGl2RenderingContext as GL, WebGlProgram, WebGlVertexArrayObject,
};

pub struct Config {
    pub selector: &'static str,
    pub pixel_ratio: f64,
}

#[wasm_bindgen]
pub struct Renderer {
    canvas: HtmlCanvasElement,
    ctx: GL,
    aspect_ratio: f32,
    shaders: HashMap<ShaderType, WebGlProgram>,
    vaos: HashMap<ShaderType, Vec<WebGlVertexArrayObject>>,
    config: Config,
}

impl Renderer {
    pub fn new(config: Config) -> Self {
        let mut canvas = get_canvas(config.selector);
        let aspect_ratio = resize_canvas(&mut canvas, config.pixel_ratio);
        let ctx = canvas.get_context("webgl2").expect("Can't create webgl2 context. Make sure your browser supports WebGL2").unwrap().dyn_into::<GL>().unwrap();

        let mut shaders = HashMap::new();
        shaders.insert(ShaderType::Color, create_color_program(&ctx).expect("Can't create color shader!"));
        shaders.insert(ShaderType::VertexColor, create_vertex_color_program(&ctx).expect("Can't create vertex color shader!"));
        shaders.insert(ShaderType::Texture, create_texture_program(&ctx).expect("can't create texture shader!"));
        let vaos = HashMap::new();
        Self {
            canvas,
            ctx,
            aspect_ratio,
            vaos,
            shaders,
            config,
        }
    }
    pub fn setup_renderer(&mut self, storage: &Storage) {
        for each_type in ShaderType::iter() {
            if let Some(meshes) = storage.get_meshes(&each_type) {
                let program = self
                    .shaders
                    .get(&each_type)
                    .expect("Can't find the program!");
                let mut vaos = Vec::new();
                for mesh in meshes {
                    let vao = self.ctx.create_vertex_array().expect("Can't creat VAO");
                    self.ctx.bind_vertex_array(Some(&vao));
                    bind_buffer_and_attribute(
                        &self.ctx,
                        &program,
                        "position",
                        &mesh.geometry.vertices,
                        3,
                    )
                    .expect("Can't bind postion");
                    bind_buffer_and_attribute(
                        &self.ctx,
                        &program,
                        "normal",
                        &mesh.geometry.normals,
                        3,
                    )
                    .expect("Can't bind normals");
                    if mesh.material.shader_type == ShaderType::VertexColor {
                        bind_buffer_and_attribute(
                            &self.ctx,
                            &program,
                            "color",
                            mesh.material
                                .vertex_colors
                                .as_ref()
                                .expect("Expected vertex color, found nothing!"),
                            4,
                        )
                        .expect("Couldn't bind vertex colors.");
                    } else if mesh.material.shader_type == ShaderType::Texture {
                        bind_buffer_and_attribute(
                            &self.ctx,
                            &program,
                            "texCoord",
                            mesh.material
                                .tex_coords
                                .as_ref()
                                .expect("Expected texture coordinates, found nothing!"),
                            2,
                        )
                        .expect("Couldn't bind tex coordinates");
                    }
                    self.ctx.bind_vertex_array(None);
                    self.ctx.bind_buffer(GL::ARRAY_BUFFER, None);
                    vaos.push(vao);
                }
                self.vaos.insert(each_type, vaos);
            }
        }
        self.ctx.clear_color(0.0, 0.0, 0.0, 1.0);
        self.ctx.clear_depth(1.0);
        self.ctx.depth_func(GL::LEQUAL);
        self.ctx.enable(GL::DEPTH_TEST);
        self.ctx.front_face(GL::CCW);
        self.ctx.cull_face(GL::BACK);
        self.ctx.enable(GL::CULL_FACE);
    }
    pub fn render(&self, storage: &Storage) {
        self.ctx.clear(GL::COLOR_BUFFER_BIT | GL::DEPTH_BUFFER_BIT);
        for each_type in ShaderType::iter() {
            let program = self.shaders.get(&each_type).unwrap();
            self.ctx.use_program(Some(&program));
            if let Some(meshes) = storage.get_meshes(&each_type) {
                let transforms = storage.get_transforms(&each_type).unwrap();
                let vaos = self.vaos.get(&each_type).expect("No vao found!");
                for (i, mesh) in meshes.iter().enumerate() {
                    let vao = &vaos[i];
                    self.ctx.bind_vertex_array(Some(&vao));
                    if each_type == ShaderType::Color {
                        bind_uniform_vec4(
                            &self.ctx,
                            program,
                            "color",
                            &mesh
                                .material
                                .color
                                .expect("Can't render a color materaial without a color!"),
                        );
                    } else if each_type == ShaderType::Texture {
                        self.ctx.active_texture(GL::TEXTURE0);
                        bind_uniform_i32(&self.ctx, program, "sampler", 0);
                    }
                    let model = Matrix4::from(transforms[i]);
                    bind_uniform_mat4(&self.ctx, program, "model", &model);
                    let normal_matrix = model.invert().unwrap().transpose();
                    bind_uniform_mat4(&self.ctx, program, "normalMatrix", &normal_matrix);
                    let indices = &mesh.geometry.indices;
                    bind_index_buffer(&self.ctx, &indices).expect("Can't bind index buffer!");
                    self.ctx.draw_elements_with_i32(
                        GL::TRIANGLES,
                        indices.len() as i32,
                        GL::UNSIGNED_SHORT,
                        0,
                    );
                    self.ctx.bind_vertex_array(None);
                    self.ctx.bind_buffer(GL::ARRAY_BUFFER, None);
                    self.ctx.bind_buffer(GL::ELEMENT_ARRAY_BUFFER, None);
                }
            } else {
                continue;
            }
            self.ctx.use_program(None);
        }
    }
    pub fn update_viewport(&mut self, viewport: &Viewport) {
        for (_, program) in &self.shaders {
            self.ctx.use_program(Some(&program));
            bind_uniform_mat4(&self.ctx, &program, "view", &viewport.view);
            bind_uniform_mat4(&self.ctx, &program, "proj", &viewport.proj);
            self.ctx.use_program(None);
        }
    }
    pub fn resize(&mut self, viewport: &mut Viewport) {
        self.aspect_ratio = resize_canvas(&mut self.canvas, self.config.pixel_ratio);
        use cgmath::{perspective, Deg, Rad};
        let proj = perspective(Rad::from(Deg(60.)), self.aspect_ratio, 0.1, 100.);
        self.ctx.viewport(
            0,
            0,
            self.canvas.width() as i32,
            self.canvas.height() as i32,
        );
        for (_, program) in &self.shaders {
            self.ctx.use_program(Some(&program));
            bind_uniform_mat4(&self.ctx, &program, "proj", &proj);
            self.ctx.use_program(None);
        }
        viewport.proj = proj;
    }
    pub fn get_context(&self) -> &GL {
        &self.ctx
    }
    pub fn aspect_ratio(&self) -> f32 {
        self.aspect_ratio
    }
}
