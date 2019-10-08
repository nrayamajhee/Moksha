use super::shader::{
    bind_buffer_and_attribute, bind_index_buffer, bind_uniform_i32, bind_uniform_mat4,
    bind_uniform_vec4, bind_uniform_vec3, create_color_program, create_simple_program, create_texture_program,
    create_vertex_color_program, ShaderType, create_wire_program
};
use crate::dom_factory::{get_canvas, resize_canvas};
use crate::{
    controller::Viewport,
    log,
    scene::{Scene},
    mesh::Mesh,
};
use std::collections::HashMap;
use wasm_bindgen::{prelude::*, JsCast};
use web_sys::{
    HtmlCanvasElement, HtmlElement, WebGl2RenderingContext as GL, WebGlProgram,
    WebGlVertexArrayObject,
};

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum DrawMode {
    Points,
    Wireframe,
    Lines,
    PointyLines,
    Triangle,
    TriangleNoDepth,
}

#[derive(Debug)]
pub struct Config {
    pub selector: &'static str,
    pub pixel_ratio: f64,
}

/// WebGL renderer that compiles, binds and executes all shaders; also capable of handling window resizes and configuration changes
#[wasm_bindgen]
#[derive(Debug)]
pub struct Renderer {
    canvas: HtmlCanvasElement,
    ctx: GL,
    aspect_ratio: f32,
    shaders: HashMap<ShaderType, WebGlProgram>,
    config: Config,
}

impl Renderer {
    pub fn new(config: Config) -> Self {
        let mut canvas = get_canvas(config.selector);
        let aspect_ratio = resize_canvas(&mut canvas, config.pixel_ratio);
        let ctx = canvas
            .get_context("webgl2")
            .expect("Can't create webgl2 context. Make sure your browser supports WebGL2")
            .unwrap()
            .dyn_into::<GL>()
            .unwrap();

        let mut shaders = HashMap::new();
        shaders.insert(
            ShaderType::Simple,
            create_simple_program(&ctx).expect("Can't create simple shader!"),
        );
        shaders.insert(
            ShaderType::Color,
            create_color_program(&ctx).expect("Can't create color shader!"),
        );
        shaders.insert(
            ShaderType::VertexColor,
            create_vertex_color_program(&ctx).expect("Can't create vertex color shader!"),
        );
        shaders.insert(
            ShaderType::Texture,
            create_texture_program(&ctx).expect("can't create texture shader!"),
        );
        shaders.insert(
            ShaderType::Wireframe,
            create_wire_program(&ctx).expect("can't create texture shader!"),
        );
        log!("Renderer created");
        Self {
            canvas,
            ctx,
            aspect_ratio,
            shaders,
            config,
        }
    }
    pub fn create_vao(&self, mesh: &Option<Mesh>) -> Option<WebGlVertexArrayObject> {
        if let Some(mesh) = mesh {
            let shader_type = mesh.material.shader_type;
            let program = self
                .shaders
                .get(&shader_type)
                .expect("Can't find the program!");
            let vao = self.ctx.create_vertex_array().expect("Can't creat VAO");
            self.ctx.bind_vertex_array(Some(&vao));
            // bind vertices
            if shader_type == ShaderType::Wireframe{
                let mut vertices = Vec::new();
                let mut bary_buffer = Vec::new();
                let barycentric: [f32;9] = [1.0, 0.0,0.0,0.0,1.0,0.0,0.0,0.0,1.0];
                for each in mesh.geometry.indices.iter() {
                    let i = (each * 3) as usize;
                    vertices.push(mesh.geometry.vertices[i]);
                    vertices.push(mesh.geometry.vertices[i+1]);
                    vertices.push(mesh.geometry.vertices[i+2]);
                }
                for _ in 0..vertices.len() / 9 {
                    for each in &barycentric {
                        bary_buffer.push(*each);
                    }
                }
                bind_buffer_and_attribute(
                    &self.ctx,
                    &program,
                    "position",
                    &vertices,
                    3,
                )
                .expect("Can't bind postion");
                bind_buffer_and_attribute(
                    &self.ctx,
                    &program,
                    "barycentric",
                    &bary_buffer[..],
                    3,
                ).expect("Can't bind postion");
            } else {
                bind_buffer_and_attribute(
                    &self.ctx,
                    &program,
                    "position",
                    &mesh.geometry.vertices,
                    3,
                )
                .expect("Can't bind postion");
            }
            // bind normals
            if shader_type != ShaderType::Simple && shader_type != ShaderType::Wireframe {
                bind_buffer_and_attribute(
                    &self.ctx,
                    &program,
                    "normal",
                    &mesh.geometry.normals,
                    3,
                )
                .expect("Can't bind normals");
            // bind vertex color
            } else if shader_type == ShaderType::VertexColor {
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
            // bind texture
            } else if shader_type == ShaderType::Texture {
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
            self.ctx.bind_buffer(GL::ARRAY_BUFFER, None);
            self.ctx.bind_vertex_array(None);
            Some(vao)
        } else {
            None
        }
    }
    pub fn setup_renderer(&self) {
        let gl = &self.ctx;
        gl.clear_color(0.1, 0.1, 0.1, 1.0);
        gl.clear_depth(1.0);
        gl.depth_func(GL::LEQUAL);
        gl.front_face(GL::CCW);
        gl.cull_face(GL::BACK);
        gl.enable(GL::CULL_FACE);
        gl.enable(GL::DEPTH_TEST);
        gl.blend_func(GL::SRC_ALPHA, GL::ONE_MINUS_SRC_ALPHA);
        log!("Renderer is ready to draw");
    }
    pub fn render(&self, scene: &Scene, viewport: &Viewport) {
        self.ctx.clear(GL::COLOR_BUFFER_BIT | GL::DEPTH_BUFFER_BIT);
        let storage = scene.storage();
        let storage = storage.borrow();
        let meshes = storage.meshes();
        for (i, mesh) in meshes.iter().enumerate() {
            if let Some(mesh) = mesh {
                let info = storage.info(i);
                if !info.render {
                    continue;
                }
                let vao = storage.vao(i);
                let shader_type = mesh.material.shader_type;
                let program = self.shaders.get(&shader_type).unwrap();
                self.ctx.use_program(Some(&program));
                self.ctx.bind_vertex_array(vao);
                if shader_type != ShaderType::VertexColor && shader_type != ShaderType::Texture {
                    bind_uniform_vec4(
                        &self.ctx,
                        program,
                        "color",
                        &mesh
                            .material
                            .color
                            .expect("Can't render a color materaial without a color!"),
                    );
                } else if shader_type == ShaderType::Texture {
                    self.ctx.active_texture(GL::TEXTURE0);
                    bind_uniform_i32(&self.ctx, program, "sampler", 0);
                }
                let model =  storage.parent_tranform(i) * storage.transform(i);
                bind_uniform_mat4(&self.ctx, program, "model", &model.to_homogeneous());
                if shader_type != ShaderType::Simple && shader_type != ShaderType::Wireframe {
                    let normal_matrix = model.inverse().to_homogeneous().transpose();
                    bind_uniform_mat4(&self.ctx, program, "inv_transpose", &normal_matrix);
                }
                bind_uniform_mat4(&self.ctx, &program, "view", &viewport.view());
                bind_uniform_mat4(&self.ctx, &program, "proj", &viewport.proj());
                let indices = &mesh.geometry.indices;
                bind_index_buffer(&self.ctx, &indices).expect("Can't bind index buffer!");
                if info.render {
                    match info.draw_mode {
                        DrawMode::Wireframe => {
                            let gl = &self.ctx;
                            gl.enable(GL::BLEND);
                            gl.depth_mask(false);
                            gl.disable(GL::CULL_FACE);
                            self.ctx.draw_arrays(
                                GL::TRIANGLES,
                                0,
                                indices.len() as i32,
                            );
                            gl.enable(GL::CULL_FACE);
                            gl.disable(GL::BLEND);
                            gl.depth_mask(true);
                        }
                        DrawMode::Triangle => {
                            self.ctx.draw_elements_with_i32(
                                GL::TRIANGLES,
                                indices.len() as i32,
                                GL::UNSIGNED_SHORT,
                                0,
                            );
                        }
                        DrawMode::TriangleNoDepth => {
                            self.ctx.disable(GL::DEPTH_TEST);
                            self.ctx.draw_elements_with_i32(
                                GL::TRIANGLES,
                                indices.len() as i32,
                                GL::UNSIGNED_SHORT,
                                0,
                            );
                            self.ctx.enable(GL::DEPTH_TEST);
                        }
                        DrawMode::Lines => {
                            self.ctx.draw_elements_with_i32(
                                GL::LINES,
                                indices.len() as i32,
                                GL::UNSIGNED_SHORT,
                                0,
                            );
                        }
                        _ => (),
                    }
                }
            }
        }
        self.ctx.bind_vertex_array(None);
        self.ctx.bind_buffer(GL::ARRAY_BUFFER, None);
        self.ctx.bind_buffer(GL::ELEMENT_ARRAY_BUFFER, None);
        self.ctx.use_program(None);
    }
    pub fn resize(&mut self) {
        log!("Renderer resized");
        self.aspect_ratio = resize_canvas(&mut self.canvas, self.config.pixel_ratio);
        // log!("New aspect ratio: {:?}", self.aspect_ratio());
        self.ctx.viewport(
            0,
            0,
            self.canvas.width() as i32,
            self.canvas.height() as i32,
        );
    }
    pub fn context(&self) -> &GL {
        &self.ctx
    }
    pub fn canvas(&self) -> &HtmlCanvasElement {
        &self.canvas
    }
    pub fn aspect_ratio(&self) -> f32 {
        self.aspect_ratio
    }
    pub fn width(&self) -> u32 {
        self.canvas.width()
    }
    pub fn height(&self) -> u32 {
        self.canvas.height()
    }
    pub fn change_cursor(&self, cursory_type: CursorType) {
        let canvas_style = self
            .canvas
            .clone()
            .dyn_into::<HtmlElement>()
            .unwrap()
            .style();
        match cursory_type {
            CursorType::Pointer => {
                canvas_style.set_property("cursor", "default").unwrap();
            }
            CursorType::Grab => {
                canvas_style.set_property("cursor", "grabbing").unwrap();
            }
            CursorType::ZoomIn => {
                canvas_style.set_property("cursor", "zoom-in").unwrap();
            }
            CursorType::ZoomOut => {
                canvas_style.set_property("cursor", "zoom-out").unwrap();
            }
        }
    }
}

pub enum CursorType {
    Pointer,
    Grab,
    ZoomIn,
    ZoomOut,
}
