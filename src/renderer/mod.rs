mod shader;
use crate::{
    controller::Viewport,
    dom_factory::{body, get_canvas, resize_canvas},
    log,
    mesh::Mesh,
    scene::Scene,
    LightType, Storage,
};
use maud::html;
use nalgebra::{UnitQuaternion, Vector3};
pub use shader::*;
use std::collections::HashMap;
use std::f32::consts::PI;
use strum::IntoEnumIterator;
use wasm_bindgen::{prelude::*, JsCast};
use web_sys::{
    HtmlCanvasElement, HtmlElement, WebGl2RenderingContext as GL, WebGlProgram,
    WebGlVertexArrayObject,
};

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum DrawMode {
    Points,
    Lines,
    Triangle,
    Arrays,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct RenderConfig {
    pub depth_fn: u32,
    pub front_face: u32,
    pub cull_face: u32,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            depth_fn: GL::LEQUAL,
            front_face: GL::CCW,
            cull_face: GL::BACK,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct RenderFlags {
    pub render: bool,
    pub depth: bool,
    pub blend: bool,
    pub cull_face: bool,
}

impl Default for RenderFlags {
    fn default() -> Self {
        Self {
            render: false,
            depth: true,
            blend: false,
            cull_face: true,
        }
    }
}

impl RenderFlags {
    pub fn no_depth() -> Self {
        Self {
            depth: false,
            ..Default::default()
        }
    }
    pub fn no_depth_blend_cull() -> Self {
        Self {
            blend: true,
            depth: false,
            cull_face: false,
            ..Default::default()
        }
    }
    pub fn blend_cull() -> Self {
        Self {
            blend: true,
            cull_face: false,
            ..Default::default()
        }
    }
}

#[derive(Debug)]
pub struct RendererConfig {
    pub id: &'static str,
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
    config: RendererConfig,
    render_config: RenderConfig,
}

impl Renderer {
    pub fn new(config: RendererConfig) -> Self {
        let dom = html! {
            canvas id=(config.id) oncontextmenu="return false;" {}
        };
        body()
            .insert_adjacent_html("beforeend", dom.into_string().as_str())
            .expect("Couldn't insert markup into the DOM!");
        let mut canvas = get_canvas(config.id);
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
            create_program(
                &ctx,
                include_str!("shaders/simple.vert"),
                include_str!("shaders/simple.frag"),
            )
            .expect("Can't create simple shader!"),
        );
        shaders.insert(
            ShaderType::Color,
            create_program(
                &ctx,
                include_str!("shaders/color.vert"),
                include_str!("shaders/color.frag"),
            )
            .expect("Can't create color shader!"),
        );
        shaders.insert(
            ShaderType::Wireframe,
            create_program(
                &ctx,
                include_str!("shaders/wire.vert"),
                include_str!("shaders/wire.frag"),
            )
            .expect("Can't create wire shader!"),
        );
        shaders.insert(
            ShaderType::VertexColor,
            create_vertex_color_program(&ctx).expect("Can't create vertex color shader!"),
        );
        log!("Renderer created");
        let render_config = Default::default();
        Self::setup_renderer(&ctx, render_config);
        Self {
            canvas,
            ctx,
            aspect_ratio,
            shaders,
            config,
            render_config,
        }
    }
    pub fn create_vao(&self, mesh: &Mesh) -> WebGlVertexArrayObject {
        let shader_type = mesh.material.shader_type;
        let program = self
            .shaders
            .get(&shader_type)
            .expect("Can't find the program!");
        let vao = self.ctx.create_vertex_array().expect("Can't creat VAO");
        self.ctx.bind_vertex_array(Some(&vao));
        // bind vertices
        if mesh.material.wire_overlay || shader_type == ShaderType::Wireframe {
            let mut vertices = Vec::new();
            let mut bary_buffer = Vec::new();
            let barycentric: [f32; 9] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
            for each in mesh.geometry.indices.iter() {
                let i = (each * 3) as usize;
                vertices.push(mesh.geometry.vertices[i]);
                vertices.push(mesh.geometry.vertices[i + 1]);
                vertices.push(mesh.geometry.vertices[i + 2]);
            }
            for _ in 0..vertices.len() / 9 {
                for each in &barycentric {
                    bary_buffer.push(*each);
                }
            }
            bind_buffer_and_attribute(&self.ctx, &program, "position", &vertices, 3)
                .expect("Can't bind postion");
            bind_buffer_and_attribute(&self.ctx, &program, "barycentric", &bary_buffer[..], 3)
                .expect("Can't bind postion");
        } else {
            bind_buffer_and_attribute(&self.ctx, &program, "position", &mesh.geometry.vertices, 3)
                .expect("Can't bind postion");
        }
        // bind normals
        if shader_type == ShaderType::Color && mesh.material.wire_overlay {
            let mut normals = Vec::new();
            for each in mesh.geometry.indices.iter() {
                let i = (each * 3) as usize;
                normals.push(mesh.geometry.normals[i]);
                normals.push(mesh.geometry.normals[i + 1]);
                normals.push(mesh.geometry.normals[i + 2]);
            }
            bind_buffer_and_attribute(&self.ctx, &program, "normal", &normals, 3)
                .expect("Can't bind normals");
        } else if shader_type != ShaderType::Simple && shader_type != ShaderType::Wireframe {
            bind_buffer_and_attribute(&self.ctx, &program, "normal", &mesh.geometry.normals, 3)
                .expect("Can't bind normals");
        }
        // bind vertex color
        if shader_type == ShaderType::VertexColor {
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
        }
        // bind texture
        if let Some(tex_coords) = mesh.material.tex_coords.as_ref() {
            bind_buffer_and_attribute(
                &self.ctx,
                &program,
                "tex_coords",
                tex_coords,
                2,
            )
            .expect("Couldn't bind tex coordinates");
        }
        self.ctx.bind_buffer(GL::ARRAY_BUFFER, None);
        self.ctx.bind_vertex_array(None);
        vao
    }
    pub fn setup_renderer(gl: &GL, render_config: RenderConfig) {
        gl.clear_color(0.1, 0.1, 0.1, 1.0);
        gl.clear_depth(1.0);
        gl.depth_func(render_config.depth_fn);
        gl.front_face(render_config.front_face);
        gl.cull_face(render_config.cull_face);
        gl.blend_func(GL::SRC_ALPHA, GL::ONE_MINUS_SRC_ALPHA);
        log!("Renderer is ready to draw");
    }
    fn setup_lights(&self, storage: &Storage) {
        let gl = &self.ctx;
        let program = self.shaders.get(&ShaderType::Color).unwrap();
        gl.use_program(Some(&program));
        let mut num_l_amb = 0;
        let mut num_l_point = 0;
        let mut num_l_dir = 0;
        let mut num_l_spot = 0;
        for light in storage.lights() {
            if !light.light {
                continue;
            }
            match light.light_type {
                LightType::Ambient => {
                    set_vec3(
                        gl,
                        program,
                        &format!("amb_lights[{}].color", num_l_amb),
                        &light.color,
                    );
                    set_f32(
                        gl,
                        program,
                        &format!("amb_lights[{}].intensity", num_l_amb),
                        light.intensity,
                    );
                    num_l_amb += 1;
                }
                LightType::Point | LightType::Directional | LightType::Spot => {
                    let (attrib, index) = if light.light_type == LightType::Point {
                        ("point_lights", num_l_point)
                    } else if light.light_type == LightType::Spot {
                        ("spot_lights", num_l_spot)
                    } else {
                        ("dir_lights", num_l_dir)
                    };
                    let position = (storage.parent_tranform(light.node_id)
                        * storage.transform(light.node_id))
                    .isometry
                    .translation
                    .vector
                    .data;
                    let range = 100.;
                    let linear = 4.5 / range;
                    let quadratic = 7.5 / (range * range);
                    set_f32(
                        gl,
                        program,
                        &format!("{}[{}].linear", attrib, index),
                        linear,
                    );
                    set_f32(
                        gl,
                        program,
                        &format!("{}[{}].quadratic", attrib, index),
                        quadratic,
                    );
                    set_vec3(
                        gl,
                        program,
                        &format!("{}[{}].position", attrib, index),
                        &position,
                    );
                    if light.light_type == LightType::Directional
                        || light.light_type == LightType::Spot
                    {
                        let vector = (storage.parent_tranform(light.node_id)
                            * storage.transform(light.node_id))
                        .isometry
                        .rotation
                        .transform_vector(&Vector3::identity());
                        // The cone and arrows mesh is intrinsically oriented 90 deg
                        let direction = UnitQuaternion::from_euler_angles(0., PI / 2., 0.)
                            .transform_vector(&vector)
                            .data;
                        set_vec3(
                            gl,
                            program,
                            &format!("{}[{}].direction", attrib, index),
                            &direction,
                        );
                    }
                    if light.light_type == LightType::Spot {
                        set_f32(
                            gl,
                            program,
                            &format!("{}[{}].cutoff", attrib, index),
                            f32::cos(PI / 30.),
                        );
                        set_f32(
                            gl,
                            program,
                            &format!("{}[{}].outer_cutoff", attrib, index),
                            f32::cos(std::f32::consts::PI / 25.),
                        );
                    }
                    set_vec3(
                        gl,
                        program,
                        &format!("{}[{}].color", attrib, index),
                        &light.color,
                    );
                    set_f32(
                        gl,
                        program,
                        &format!("{}[{}].intensity", attrib, index),
                        light.intensity,
                    );
                    if light.light_type == LightType::Point {
                        num_l_point += 1;
                    } else if light.light_type == LightType::Spot {
                        num_l_spot += 1;
                    } else {
                        num_l_dir += 1;
                    };
                }
            }
        }
        set_i32(gl, program, "num_l_amb", num_l_amb as i32);
        set_i32(gl, program, "num_l_point", num_l_point as i32);
        set_i32(gl, program, "num_l_dir", num_l_dir as i32);
        set_i32(gl, program, "num_l_spot", num_l_spot as i32);
    }
    fn update_viewport(&self, viewport: &Viewport) {
        for each in ShaderType::iter() {
            if let Some(program) = self.shaders.get(&each) {
                let gl = &self.ctx;
                gl.use_program(Some(&program));
                set_mat4(gl, &program, "view", &viewport.view());
                set_mat4(gl, &program, "proj", &viewport.proj());
                if each == ShaderType::Color {
                    set_vec3(gl, program, "eye", &viewport.eye());
                }
            }
        }
    }
    fn set_flags(gl: &GL, render_flags: RenderFlags) {
        match render_flags.blend {
            true => {
                gl.enable(GL::BLEND);
            }
            false => {
                gl.disable(GL::BLEND);
            }
        }
        match render_flags.depth {
            true => {
                gl.enable(GL::DEPTH_TEST);
            }
            false => {
                gl.disable(GL::DEPTH_TEST);
            }
        }
        match render_flags.cull_face {
            true => {
                gl.enable(GL::CULL_FACE);
            }
            false => {
                gl.disable(GL::CULL_FACE);
            }
        }
    }
    fn render_mesh(&self, storage: &Storage, i: usize) {
        let gl = &self.ctx;
        if let Some(mesh) = storage.mesh(i) {
            let info = storage.info(i);
            let vao = storage.vao(i);
            gl.bind_vertex_array(vao);
            let shader_type = mesh.material.shader_type;
            let program = self.shaders.get(&shader_type).unwrap();
            gl.use_program(Some(&program));
            if shader_type == ShaderType::Simple
                || shader_type == ShaderType::Color
                || shader_type == ShaderType::Wireframe
            {
                set_vec4(
                    gl,
                    program,
                    "color",
                    &mesh
                        .material
                        .color
                        .expect("Can't render a color materaial without a color!"),
                );
            }
            if shader_type == ShaderType::Color {
                set_bool(gl, program, "flat_shade", mesh.material.flat_shade);
                set_bool(gl, program, "wire_overlay", mesh.material.wire_overlay);
                if let Some(_) = mesh.material.tex_coords {
                    let tex_i = mesh.material.texture_indices[0];
                    let texture = storage.texture(tex_i);
                    gl.bind_texture(GL::TEXTURE_2D, Some(&texture));
                    gl.active_texture(GL::TEXTURE0);
                    set_i32(gl, program, "sampler", 0);
                    set_bool(gl, program, "has_albedo", true);
                } else {
                    set_bool(gl, program, "has_albedo", false);
                }
            }
            let model = storage.parent_tranform(i) * storage.transform(i);
            set_mat4(gl, program, "model", &model.to_homogeneous());
            let indices = &mesh.geometry.indices;
            bind_index_buffer(gl, &indices).expect("Can't bind index buffer!");

            if shader_type == ShaderType::Wireframe {
                gl.enable(GL::SAMPLE_ALPHA_TO_COVERAGE);
            } else {
                gl.disable(GL::SAMPLE_ALPHA_TO_COVERAGE);
            }
            Self::set_flags(&gl, info.render_flags);
            match info.draw_mode {
                DrawMode::Arrays => {
                    gl.draw_arrays(GL::TRIANGLES, 0, indices.len() as i32);
                }
                DrawMode::Lines => {
                    gl.draw_elements_with_i32(
                        GL::LINES,
                        indices.len() as i32,
                        GL::UNSIGNED_SHORT,
                        0,
                    );
                }
                DrawMode::Points => {
                    gl.draw_elements_with_i32(
                        GL::POINTS,
                        indices.len() as i32,
                        GL::UNSIGNED_SHORT,
                        0,
                    );
                }
                DrawMode::Triangle => {
                    gl.draw_elements_with_i32(
                        GL::TRIANGLES,
                        indices.len() as i32,
                        GL::UNSIGNED_SHORT,
                        0,
                    );
                }
            }
        }
    }
    pub fn render(&self, scene: &Scene, viewport: &Viewport) {
        let gl = &self.ctx;
        gl.clear(GL::COLOR_BUFFER_BIT | GL::DEPTH_BUFFER_BIT);
        let storage = scene.storage();
        let storage = storage.borrow();
        self.setup_lights(&storage);
        self.update_viewport(viewport);
        let len = storage.meshes().len();
        let render_stage = |condition: Box<dyn Fn(bool, Option<ShaderType>) -> bool>| {
            for i in 0..len {
                let info = storage.info(i);
                let shader_type = if let Some(mesh) = storage.mesh(i) {
                    Some(mesh.material.shader_type)
                } else {
                    None
                };
                if info.render_flags.render && condition(info.render_flags.depth, shader_type) {
                    self.render_mesh(&storage, i);
                }
            }
        };
        // render normal meshes first
        render_stage(Box::new(|depth, shader_type| {
            depth && shader_type != Some(ShaderType::Wireframe)
        }));
        // render wireframe next
        render_stage(Box::new(|depth, shader_type| {
            depth && shader_type == Some(ShaderType::Wireframe)
        }));
        // render depthless mesh last
        render_stage(Box::new(|depth, _| !depth));
        gl.bind_vertex_array(None);
        gl.bind_buffer(GL::ARRAY_BUFFER, None);
        gl.bind_buffer(GL::ELEMENT_ARRAY_BUFFER, None);
        gl.use_program(None);
    }
    pub fn resize(&mut self) {
        log!("Renderer resized");
        self.aspect_ratio = resize_canvas(&self.canvas, self.config.pixel_ratio);
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
                canvas_style
                    .set_property("cursor", "var(--cursor-auto)")
                    .unwrap();
            }
            CursorType::Grab => {
                canvas_style
                    .set_property("cursor", "var(--cursor-grab)")
                    .unwrap();
            }
        }
    }
}

pub enum CursorType {
    Pointer,
    Grab,
}
