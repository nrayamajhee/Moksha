mod shader;
use crate::{
    controller::Viewport,
    dom_factory::{body, get_canvas, resize_canvas},
    log,
    mesh::Mesh,
    scene::Scene,
    LightState, LightType, Projection, Storage, TextureType, Transform,
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
    pub view_transform: bool,
    pub cull_face: bool,
}

impl Default for RenderFlags {
    fn default() -> Self {
        Self {
            render: false,
            depth: true,
            blend: false,
            view_transform: true,
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
    pub fn no_cull() -> Self {
        Self {
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

#[derive(Serialize)]
pub struct ContextOptions {
    pub stencil: bool,
}

/// WebGL renderer that compiles, binds and executes all shaders; also capable of handling window resizes and configuration changes
#[wasm_bindgen]
#[derive(Debug)]
pub struct Renderer {
    canvas: HtmlCanvasElement,
    ctx: GL,
    aspect_ratio: f64,
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
        let stencil_param = JsValue::from_serde(&ContextOptions { stencil: true }).unwrap();
        let aspect_ratio = resize_canvas(&mut canvas, config.pixel_ratio);
        let ctx = canvas
            .get_context_with_context_options("webgl2", &stencil_param)
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
            ShaderType::CubeMap,
            create_program(
                &ctx,
                include_str!("shaders/cube.vert"),
                include_str!("shaders/cube.frag"),
            )
            .expect("Can't create cubemap shader!"),
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
        bind_buffer_and_attribute(
            &self.ctx,
            &program,
            "position",
            &mesh.geometry.vertices[..],
            3,
        )
        .expect("Can't bind postion");
        if mesh.material.wire_overlay != None || shader_type == ShaderType::Wireframe {
            let mut bary_buffer = Vec::new();
            let barycentric: [f32; 9] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
            for _ in 0..mesh.geometry.vertices.len() / 9 {
                for each in &barycentric {
                    bary_buffer.push(*each);
                }
            }
            bind_buffer_and_attribute(&self.ctx, &program, "barycentric", &bary_buffer[..], 3)
                .expect("Can't bind postion");
        }
        // bind normals
        if shader_type == ShaderType::Color {
            bind_buffer_and_attribute(&self.ctx, &program, "normal", &mesh.geometry.normals[..], 3)
                .expect("Can't bind normals");
        }
        // bind texture
        if let Some(coords) = mesh.material.tex_coords.as_ref() {
            if mesh.material.tex_type == TextureType::Tex2d {
                bind_buffer_and_attribute(&self.ctx, &program, "tex_coords", &coords[..], 2)
                    .expect("Couldn't bind tex coordinates");
            }
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
        gl.enable(GL::STENCIL_TEST);
        gl.stencil_op(GL::KEEP, GL::KEEP, GL::REPLACE);
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
            if light.state == LightState::Off {
                continue;
            }
            let (attrib, index) = match light.light_type {
                LightType::Point => ("point_lights", num_l_point),
                LightType::Spot => ("spot_lights", num_l_spot),
                LightType::Ambient => ("amb_lights", num_l_amb),
                LightType::Directional => ("dir_lights", num_l_dir),
            };
            let position = (storage.parent_transform(light.obj_id)
                * storage.transform(light.obj_id))
            .isometry
            .translation
            .vector;
            //let position = [vector.x as f32, vector.y as f32, vector.z as f32];
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
                &position.into(),
            );
            if light.light_type == LightType::Directional || light.light_type == LightType::Spot {
                let vector = (storage.parent_transform(light.obj_id)
                    * storage.transform(light.obj_id))
                .isometry
                .rotation
                .transform_vector(&Vector3::identity());
                // The cone and arrows mesh is intrinsically oriented 90 deg
                let dir =
                    UnitQuaternion::from_euler_angles(0., PI / 2., 0.).transform_vector(&vector);
                let direction = [dir.x as f32, dir.y as f32, dir.z as f32];
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
                &light.color.into(),
            );
            set_f32(
                gl,
                program,
                &format!("{}[{}].intensity", attrib, index),
                light.intensity as f32,
            );
            match light.light_type {
                LightType::Point => {
                    num_l_point += 1;
                }
                LightType::Spot => {
                    num_l_spot += 1;
                }
                LightType::Ambient => {
                    num_l_amb += 1;
                }
                LightType::Directional => {
                    num_l_dir += 1;
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
                if each == ShaderType::CubeMap {
                    let rotation: nalgebra::Matrix4<f32> =
                        viewport.isometry().rotation.to_homogeneous().into();
                    set_mat4(gl, &program, "view", &rotation);
                } else {
                    set_mat4(gl, &program, "view", &viewport.view());
                }
                if each == ShaderType::CubeMap
                    && viewport.projection_type() == Projection::Orthographic
                {
                    set_mat4(
                        gl,
                        &program,
                        "proj",
                        &viewport.get_proj(Projection::Perspective),
                    );
                } else {
                    set_mat4(gl, &program, "proj", &viewport.proj());
                }
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
                gl.enable(GL::STENCIL_TEST);
            }
            flse => {
                gl.disable(GL::STENCIL_TEST);
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
                        .expect("Can't render a color materaial without a color!").into(),
                );
            }
            if shader_type == ShaderType::Color {
                set_bool(gl, program, "flat_shade", mesh.material.flat_shade);
                set_bool(gl, program, "blinn_shade", true);
                if let Some(color) = mesh.material.wire_overlay {
                    let w_color: [f32; 4] = color.into();
                    set_bool(gl, program, "wire_overlay", true);
                    set_vec4(gl, program, "wire_color", &w_color.into());
                } else {
                    set_bool(gl, program, "wire_overlay", false);
                }
                if !mesh.material.texture_indices.is_empty() {
                    set_bool(gl, program, "has_albedo", true);
                    let tex_i = mesh.material.texture_indices[0];
                    let texture = storage.texture(tex_i);
                    gl.active_texture(GL::TEXTURE0);
                    gl.bind_texture(GL::TEXTURE_2D, Some(&texture));
                    set_i32(gl, program, "sampler", 0);
                } else {
                    set_bool(gl, program, "has_albedo", false);
                }
            }
            if shader_type == ShaderType::CubeMap {
                let tex_i = mesh.material.texture_indices[0];
                let texture = storage.texture(tex_i);
                gl.active_texture(GL::TEXTURE0);
                gl.bind_texture(GL::TEXTURE_CUBE_MAP, Some(&texture));
                set_i32(gl, program, "sampler", 0);
            }
            let model = storage.parent_transform(i) * storage.transform(i);
            if shader_type != ShaderType::CubeMap {
                set_mat4(gl, program, "model", &model.to_homogeneous());
            }
            let indices = &mesh.geometry.indices;
            bind_index_buffer(gl, &indices).expect("Can't bind index buffer!");

            if shader_type == ShaderType::Wireframe {
                gl.enable(GL::SAMPLE_ALPHA_TO_COVERAGE);
                set_f32(gl, &program, "width", 1.0);
                set_f32(gl, &program, "feather", 0.5);
                set_bool(gl, program, "drawing_points", false);
            } else {
                gl.disable(GL::SAMPLE_ALPHA_TO_COVERAGE);
            }
            Self::set_flags(&gl, info.render_flags);
            if mesh.material.outline != None && shader_type != ShaderType::Wireframe {
                // first pass for object outline
                gl.stencil_func(GL::ALWAYS, 1, 0xFF);
                gl.stencil_mask(0xFF);
            } else {
                gl.stencil_func(GL::ALWAYS, 1, 0xFF);
                gl.stencil_mask(0x00);
            }
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
            if let Some(scale) = mesh.material.outline {
                // second pass for drawing outlines
                if shader_type == ShaderType::Wireframe {
                    set_f32(gl, &program, "width", 3.0);
                    set_vec4(gl, &program, "color", &[1., 1., 0., 0.8]);
                    gl.draw_arrays(GL::TRIANGLES, 0, indices.len() as i32);
                } else {
                    gl.stencil_func(GL::NOTEQUAL, 1, 0xFF);
                    gl.stencil_mask(0x00);
                    let program = self.shaders.get(&ShaderType::Simple).unwrap();
                    gl.use_program(Some(&program));
                    let o_s = mesh.material.outline.unwrap() / 100.;
                    let mut t = storage.transform(i);
                    let t_s = &t.scale;
                    t.scale = Vector3::new(t_s.x + o_s, t_s.y + o_s, t_s.z + o_s);
                    let model = storage.parent_transform(i) * t;
                    set_mat4(gl, &program, "model", &model.to_homogeneous());
                    set_vec4(gl, &program, "color", &[1., 1., 0., 1.]);
                    let indices = &mesh.geometry.indices;
                    bind_index_buffer(gl, &indices).expect("Can't bind index buffer!");
                    match storage.info(i).draw_mode {
                        DrawMode::Arrays => {
                            gl.draw_arrays(GL::TRIANGLES, 0, indices.len() as i32);
                        }
                        _ => {
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
        } else {
            log!("There's no mesh at the given index. So won't render anything");
        }
    }
    pub fn render(&self, storage: &Storage, viewport: &Viewport) {
        let gl = &self.ctx;
        gl.clear(GL::COLOR_BUFFER_BIT | GL::DEPTH_BUFFER_BIT | GL::STENCIL_BUFFER_BIT);
        self.setup_lights(&storage);
        let len = storage.meshes().len();
        self.update_viewport(viewport);
        let render_stage = |condition: Box<dyn Fn(&RenderFlags, ShaderType) -> bool>| {
            for i in 0..len {
                let info = storage.info(i);
                if let Some(mesh) = storage.mesh(i) {
                    if info.render_flags.render
                        && condition(&info.render_flags, mesh.material.shader_type)
                    {
                        self.render_mesh(&storage, i);
                    }
                };
            }
        };
        // render meshes that have depth first
        render_stage(Box::new(|r_f, s_t| r_f.depth && s_t != ShaderType::CubeMap));
        //// render cubemap
        render_stage(Box::new(|r_f, shader_type| {
            shader_type == ShaderType::CubeMap
        }));
        // render depthless mesh
        render_stage(Box::new(|r_f, _| !r_f.depth));
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
    pub fn aspect_ratio(&self) -> f64 {
        self.aspect_ratio
    }
    pub fn width(&self) -> u32 {
        self.canvas.width()
    }
    pub fn height(&self) -> u32 {
        self.canvas.height()
    }
}
