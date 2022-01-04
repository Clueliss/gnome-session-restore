use include_js::JSTemplate;
use serde::{Deserialize, Serialize};

#[derive(Serialize, JSTemplate)]
#[include_js(template = "src/js/move_window_by_class.js.handlebars")]
pub(super) struct SetGeomTemplate {
    window_class: String,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    minimized: bool,
}

impl SetGeomTemplate {
    pub(super) fn from_geom<S: Into<String>>(window_class: S, g: WindowGeom) -> Self {
        SetGeomTemplate {
            window_class: window_class.into(),
            x: g.x,
            y: g.y,
            width: g.width,
            height: g.height,
            minimized: g.minimized,
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, Copy)]
pub struct WindowGeom {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub minimized: bool,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct MetaWindow {
    pub window_class: String,
    pub geom: WindowGeom,
    pub pid: i32,
    pub stable_seq: u32,
    pub gtk_app_id: Option<String>,
}
