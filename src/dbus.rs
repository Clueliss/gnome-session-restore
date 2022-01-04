use serde::{Deserialize, Serialize};
use zbus::dbus_proxy;
use zvariant::derive::Type;

#[dbus_proxy(
    interface = "com.github.clueliss.WindowCtl",
    default_service = "org.gnome.Shell",
    default_path = "/com/github/clueliss/WindowCtl"
)]
pub trait WindowCtl {
    fn get_num_monitors(&self) -> zbus::Result<u32>;
    fn list_windows(&self) -> zbus::Result<Vec<MetaWindow>>;
    fn set_window_geom_by_class(
        &self,
        window_class: &str,
        window_geom: WindowGeom,
    ) -> zbus::Result<bool>;
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Type)]
pub struct WindowGeom {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub minimized: bool,
}

#[derive(Debug, Deserialize, Serialize, Type)]
pub struct MetaWindow {
    pub geom: WindowGeom,
    pub pid: i32,
    pub stable_seq: u32,
    pub window_class: String,
    pub gtk_app_id: String,
    pub sandboxed_app_id: String,
}
