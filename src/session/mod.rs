mod find_command;

use std::{
    collections::HashSet, fs::File, hash::Hash, path::Path, process::Command, time::Duration,
};

use gio::{prelude::AppInfoExt, AppLaunchContext};
use serde::{Deserialize, Serialize};

use crate::{
    dbus::{MetaWindow, WindowCtlProxy},
    session::find_command::Exec,
};

#[derive(Serialize, Deserialize, Debug)]
struct SessionApplication {
    #[serde(flatten)]
    window: MetaWindow,
    exec: Exec,
}

#[derive(Serialize, Deserialize, Debug)]
struct Session {
    applications: Vec<SessionApplication>,
    num_monitors: u32,
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
        self.0.window.window_class.hash(state);
    }
}

fn unique_applications(sess: Vec<SessionApplication>) -> HashSet<SessionApplicationByWindowClass> {
    sess.into_iter()
        .map(SessionApplicationByWindowClass)
        .collect()
}

pub fn save<P: AsRef<Path>>(
    conn: &WindowCtlProxy,
    path: P,
    min_wm_class_sim: f64,
) -> Result<(), Box<dyn std::error::Error>> {
    let num_monitors = conn.get_num_monitors()?;

    let res = conn.list_windows()?;

    let v: Vec<_> = res
        .into_iter()
        .filter_map(|w| {
            let app_id = w.gtk_app_id.clone();

            find_command::find_command(min_wm_class_sim, w.pid, &w.window_class, &w.gtk_app_id, &w.sandboxed_app_id)
                .map(|exec| SessionApplication { window: w, exec })
                .map_err(|e| eprintln!("unable to find command for {:?}: {:?}", app_id, e))
                .ok()
        })
        .collect();

    let session = Session {
        applications: v,
        num_monitors,
    };

    let f = File::create(path)?;
    serde_json::to_writer(f, &session)?;

    Ok(())
}

pub fn restore<P: AsRef<Path>>(
    conn: &WindowCtlProxy,
    path: P,
    rm: bool,
    mark: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let f = File::open(path.as_ref())?;

    let sess: Session = serde_json::from_reader(f)?;

    let uniq = unique_applications(sess.applications);

    for SessionApplicationByWindowClass(win) in &uniq {
        match &win.exec {
            Exec::CmdLine(cmdline) => {
                let res = Command::new(&cmdline[0]).args(&cmdline[1..]).spawn();

                if let Err(e) = res {
                    eprintln!("Error spawning process '{:?}': {e}", cmdline, e = e);
                }
            }
            Exec::DesktopFile(path) => match gio::DesktopAppInfo::from_filename(path) {
                Some(x) => {
                    if let Err(e) = x.launch_uris::<AppLaunchContext>(&[], None) {
                        eprintln!("Error spawning process '{:?}': {e}", path, e = e);
                    }
                }
                None => eprintln!(
                    "Error spawning process '{:?}': could not get desktop app info",
                    path
                ),
            },
        }
    }

    std::thread::sleep(Duration::from_secs(1));

    let cur_num_monitors = conn.get_num_monitors();

    if matches!(cur_num_monitors, Ok(n) if n == sess.num_monitors) {
        for win in uniq {
            if !win.0.window.window_class.is_empty() {
                if let Err(e) =
                    conn.set_window_geom_by_class(&win.0.window.window_class, win.0.window.geom)
                {
                    eprintln!(
                        "Error moving window '{class}': {e:?}",
                        class = win.0.window.window_class,
                        e = e
                    );
                }
            }
        }
    }

    let now = chrono::Utc::now().format("%+").to_string();

    if mark {
        let new_file = path.as_ref().with_extension(now);
        std::fs::rename(path.as_ref(), &new_file)?;

        if rm {
            std::fs::remove_file(new_file)?;
        }
    } else if rm {
        std::fs::remove_file(path)?;
    }

    Ok(())
}
