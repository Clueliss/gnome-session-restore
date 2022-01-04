mod find_command;

use crate::gdbus::{GnomeShellDBusProxy, MetaWindow, WindowGeom};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet, fs::File, hash::Hash, path::Path, process::Command, time::Duration,
};

#[derive(Serialize, Deserialize, Debug)]
struct SessionApplication {
    #[serde(flatten)]
    window: MetaWindow,

    cmdline: Vec<String>,
}

struct SessionApplicationByWindowClass(SessionApplication);

impl PartialEq for SessionApplicationByWindowClass {
    fn eq(&self, other: &Self) -> bool {
        self.0.window.window_class == other.0.window.window_class
    }
}

impl Eq for SessionApplicationByWindowClass {}

impl Hash for SessionApplicationByWindowClass {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.window.window_class.hash(state)
    }
}

fn unique_applications(sess: Vec<SessionApplication>) -> HashSet<SessionApplicationByWindowClass> {
    sess.into_iter()
        .map(SessionApplicationByWindowClass)
        .collect()
}

pub fn save_session<P: AsRef<Path>>(conn: &GnomeShellDBusProxy, path: P) {
    let res = conn
        .list_all_windows()
        .expect("failed to communicate on dbus");

    let v: Vec<_> = res
        .into_iter()
        .map(|w| {
            let cmdline =
                find_command::find_command(w.pid, &w.window_class, w.gtk_app_id.as_deref())
                    .expect("failed to find corresponding command for application");
            SessionApplication { window: w, cmdline }
        })
        .collect();

    let f = File::create(path).expect("could not create session file");
    serde_json::to_writer(f, &v).expect("could not write output to file");
}

pub fn restore_session<P: AsRef<Path>>(conn: &GnomeShellDBusProxy, path: P, rm: bool, mark: bool) {
    let f = File::open(path.as_ref()).expect("could not open session file for reading");

    let sess: Vec<SessionApplication> =
        serde_json::from_reader(f).expect("could not parse session file");

    let uniq = unique_applications(sess);

    for win in &uniq {
        let res = Command::new(&win.0.cmdline[0])
            .args(&win.0.cmdline[1..])
            .spawn();

        if let Err(e) = res {
            eprintln!("Error spawning process '{:?}': {e}", win.0.cmdline, e = e);
        }
    }

    std::thread::sleep(Duration::from_secs(1));

    let n_monitors = conn.get_n_monitors().unwrap_or(2);

    for win in uniq {
        let x = if n_monitors < 2 {
            let max_x = win.0.window.geom.x + win.0.window.geom.width;
            win.0.window.geom.x - (max_x - 1920)
        } else {
            win.0.window.geom.x
        };

        let new_geom = WindowGeom {
            x,
            ..win.0.window.geom
        };

        if let Err(e) = conn.set_window_geom_by_class(&win.0.window.window_class, new_geom) {
            eprintln!(
                "Error moving window '{class}': {e}",
                class = win.0.window.window_class,
                e = e
            );
        }
    }

    let now = chrono::Utc::now().format("%+").to_string();

    if mark {
        let new_file = path.as_ref().with_extension(now);
        std::fs::rename(path.as_ref(), &new_file).expect("could not rename session file");

        if rm {
            std::fs::remove_file(new_file).expect("could not remove session file");
        }
    } else if rm {
        std::fs::remove_file(path).expect("could not remove session file");
    }
}
