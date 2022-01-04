use zbus::{Connection, Proxy};

use serde::Deserialize;

use include_js::{include_js, JSStr, JSTemplate};
use thiserror::Error;

pub mod meta_window;
pub use meta_window::{MetaWindow, WindowGeom};

const GNOME_SHELL_DEST: &str = "org.gnome.Shell";
const GNOME_SHELL_PATH: &str = "/org/gnome/Shell";

#[derive(Debug, Error)]
pub enum Error {
    #[error("dbus error")]
    DBusError(#[from] zbus::Error),

    #[error("shell eval error: {0}")]
    ShellError(String),

    #[error("internal error, failed to deserialize")]
    DeserializeError(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

pub struct GnomeShellDBusProxy<'a> {
    dbus_conn: Proxy<'a>,
}

impl<'a> GnomeShellDBusProxy<'a> {
    pub fn new(connection: &'a Connection) -> Result<Self> {
        Ok(GnomeShellDBusProxy {
            dbus_conn: Proxy::new(
                connection,
                GNOME_SHELL_DEST,
                GNOME_SHELL_PATH,
                GNOME_SHELL_DEST,
            )?,
        })
    }
}

impl<'a> GnomeShellDBusProxy<'a> {
    pub fn eval_raw<JS>(&self, js: JS) -> Result<String>
    where
        JS: AsRef<JSStr>,
    {
        let (success, res): (bool, String) =
            self.dbus_conn.call("Eval", &(js.as_ref().as_str(),))?;

        if success {
            Ok(res)
        } else {
            Err(Error::ShellError(res))
        }
    }

    pub fn eval_no_ret<JS>(&self, js: JS) -> Result<()>
    where
        JS: AsRef<JSStr>,
    {
        self.eval_raw(js).and_then(|s| {
            if s.is_empty() {
                Ok(())
            } else {
                Err(Error::ShellError(format!(
                    "expected empty return but got value: '{}'",
                    s
                )))
            }
        })
    }

    pub fn eval<R, JS>(&self, js: JS) -> Result<R>
    where
        R: for<'de> Deserialize<'de>,
        JS: AsRef<JSStr>,
    {
        self.eval_raw(js)
            .and_then(|res| serde_json::from_str(&res).map_err(Into::into))
    }

    pub fn get_n_monitors(&self) -> Result<u32> {
        const CMD: &JSStr = include_js!("src/js/get_n_monitors.js");
        self.eval(CMD)
    }

    pub fn list_all_windows(&self) -> Result<Vec<MetaWindow>> {
        const CMD: &JSStr = include_js!("src/js/list_all_windows.js");
        let meta_windows = self.eval(CMD)?;

        Ok(meta_windows)
    }

    pub fn set_window_geom_by_class(&self, window_class: &str, new_geom: WindowGeom) -> Result<()> {
        let cmd = meta_window::SetGeomTemplate::from_geom(window_class, new_geom).render_template();
        self.eval_no_ret(&cmd)?;
        Ok(())
    }
}
