mod find_command;

use crate::gdbus::{GnomeShellDBusProxy, MetaWindow};
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

pub fn save_session<P: AsRef<Path>>(
    conn: &GnomeShellDBusProxy,
    path: P,
) -> Result<(), Box<dyn std::error::Error>> {
    let num_monitors = conn.get_n_monitors()?;

    let res = conn.list_all_windows()?;

    let v: Vec<_> = res
        .into_iter()
        .filter_map(|w| {
            let app_id = w.gtk_app_id.clone();

            find_command::find_command(w.pid, w.window_class.as_deref(), w.gtk_app_id.as_deref())
                .map(|cmdline| SessionApplication { window: w, cmdline })
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

pub fn restore_session<P: AsRef<Path>>(
    conn: &GnomeShellDBusProxy,
    path: P,
    rm: bool,
    mark: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let f = File::open(path.as_ref())?;

    let sess: Session = serde_json::from_reader(f)?;

    let uniq = unique_applications(sess.applications);

    for win in &uniq {
        let res = Command::new(&win.0.cmdline[0])
            .args(&win.0.cmdline[1..])
            .spawn();

        if let Err(e) = res {
            eprintln!("Error spawning process '{:?}': {e}", win.0.cmdline, e = e);
        }
    }

    std::thread::sleep(Duration::from_secs(1));

    let cur_num_monitors = conn.get_n_monitors();

    if matches!(cur_num_monitors, Ok(n) if n == sess.num_monitors) {
        for win in uniq {
            if let Some(window_class) = &win.0.window.window_class {
                if let Err(e) = conn.set_window_geom_by_class(window_class, win.0.window.geom) {
                    eprintln!(
                        "Error moving window '{class}': {e:?}",
                        class = window_class,
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
