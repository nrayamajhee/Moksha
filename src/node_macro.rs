#[macro_export]
macro_rules! node_from_obj {
    ($scene: expr, $dir: expr, $file: expr) => {{
        $scene.object_from_obj(
            Some($dir),
            include_str!(concat!($dir, "/", $file, ".obj")),
            Some(include_str!(concat!($dir, "/", $file, ".mtl"))),
        )
    }};
    ($scene: expr, $dir: expr, $file: expr, $has_mat: expr) => {{
        if $has_mat {
            node_from_obj!($scene, $dir, $file)
        } else {
            $scene.object_from_obj(Some($dir), include_str!(concat!($dir, "/", $file, ".obj")), None)
        }
    }};
}
#[macro_export]
macro_rules! node {
    ($scene: expr, $mesh: expr, $($x:expr),*) => {
        {
            let node = $scene.from_mesh($mesh);
            use std::any::Any;
            use crate::{ ObjectInfo, renderer::{DrawMode, RenderFlags}};
            $(
                if let Some(name) = (&$x as &dyn Any).downcast_ref::<&str>() {
                    let mut info = node.info();
                    info.name = String::from(*name);
                    node.set_info(info);
                } else if let Some(name) = (&$x as &dyn Any).downcast_ref::<String>() {
                    let mut info = node.info();
                    info.name = String::from(name);
                    node.set_info(info);
                } else if let Some(info) = (&$x as &dyn Any).downcast_ref::<ObjectInfo>() {
                    node.set_info(info.to_owned());
                } else if let Some(mode) = (&$x as &dyn Any).downcast_ref::<DrawMode>() {
                    let mut info = node.info();
                    info.draw_mode = *mode;
                    node.set_info(info);
                } else if let Some(flags) = (&$x as &dyn Any).downcast_ref::<RenderFlags>() {
                    let mut info = node.info();
                    info.render_flags = *flags;
                    node.set_info(info);
                }
            )*
            node
        }
    }
}
