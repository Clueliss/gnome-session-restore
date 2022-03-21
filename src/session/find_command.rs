use std::{
    collections::HashSet,
    ffi::{OsStr, OsString},
    lazy::SyncLazy,
    os::unix::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
};

use crate::dbus::MetaWindow;
use regex::Regex;
use serde::{Deserialize, Serialize};
use thiserror::Error;

static DESKTOP_ENTRY_LOCATIONS: SyncLazy<HashSet<PathBuf>> = SyncLazy::new(|| {
    let bd = xdg::BaseDirectories::new().unwrap();

    std::iter::once(bd.get_data_home())
        .chain(bd.get_data_dirs())
        .filter_map(|mut p| {
            p.push("applications");

            if p.exists() {
                Some(p)
            } else {
                eprintln!("Ignoring {p:?} reason: directory does not exist");
                None
            }
        })
        .collect()
});

#[derive(Error, Debug)]
pub enum FindError {
    #[error("io error")]
    IOError(#[from] std::io::Error),

    #[error("could not find a suitable entry")]
    NoSuitableEntryFound,

    #[error("process is zombie")]
    ProcessIsZombie,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Exec {
    CmdLine(Vec<OsString>),
    DesktopFile(PathBuf),
}

#[derive(Debug, Copy, Clone)]
pub enum WindowClassProvider<'a> {
    Single(&'a str),
    WithAlternative(&'a str, &'a str),
}

pub fn try_find_command_by_gtk_app_id(gtk_app_id: &str) -> Result<Exec, FindError> {
    let desktop_file_name = format!("{gtk_app_id}.desktop");
    let p = Path::new("/usr/share/applications").join(&desktop_file_name);

    if p.exists() {
        Ok(Exec::DesktopFile(p))
    } else {
        Err(FindError::NoSuitableEntryFound)
    }
}

pub fn try_find_command_by_sandboxed_app_id(sandboxed_app_id: &str) -> Result<Exec, FindError> {
    let desktop_file_name = format!("{sandboxed_app_id}.desktop");

    let p = DESKTOP_ENTRY_LOCATIONS.iter().find_map(|p| {
        let p = p.join(&desktop_file_name);
        p.exists().then_some(p)
    });

    match p {
        Some(p) => Ok(Exec::DesktopFile(p)),
        None => Err(FindError::NoSuitableEntryFound),
    }
}

pub fn try_find_command_by_window_class(
    window_class: WindowClassProvider<'_>,
) -> Result<Exec, FindError> {
    let re = match window_class {
        WindowClassProvider::Single(w_class) => Regex::new(&format!(
            r#"{window_class}(-.*?)*?\.desktop"#,
            window_class = regex::escape(&w_class.to_lowercase())
        )),
        WindowClassProvider::WithAlternative(w_class, alt_w_class) => Regex::new(&format!(
            r#"({window_class}|{alt_window_class})(-.*?)*?\.desktop"#,
            window_class = regex::escape(&w_class.to_lowercase()),
            alt_window_class = regex::escape(&alt_w_class.to_lowercase())
        )),
    }
    .unwrap();

    let desktop_file = DESKTOP_ENTRY_LOCATIONS
        .iter()
        .filter_map(|location| std::fs::read_dir(location).ok())
        .flatten()
        .filter_map(|file| match file {
            Err(_) => None,
            Ok(file) => {
                let filename = file.file_name();
                let lowercase_filename = filename.to_string_lossy().as_ref().to_lowercase();

                re.is_match(&lowercase_filename).then_some(file)
            }
        })
        .min_by_key(|direntry| {
            let filename = direntry.file_name();
            let filename = filename.to_string_lossy();

            match window_class {
                WindowClassProvider::Single(class) => strsim::levenshtein(class, filename.as_ref()),
                WindowClassProvider::WithAlternative(class_a, class_b) => {
                    let sim_a = strsim::levenshtein(class_a, filename.as_ref());
                    let sim_b = strsim::levenshtein(class_b, filename.as_ref());
                    (sim_a + sim_b) / 2
                }
            }
        });

    match desktop_file {
        Some(direntry) => Ok(Exec::DesktopFile(direntry.path())),
        None => Err(FindError::NoSuitableEntryFound),
    }
}

/// Tries to get the commandline for a given pid from the `/proc` filesystem.
///
/// # Disclaimer
/// While this may sound simple, it really is not.
///
/// ## /proc/{pid}/cmdline
/// Normally it is expected that `/proc/{pid}/cmdline` is seperated by \0 characters.
/// So parsing would be simple.
/// Except that it sometimes isn't because processes can arbitrarily write to `argv`. So:
///
/// 1. Sometimes it is seperated by spaces and all arguments are stuffed into `argv[0]`.
/// 2. Sometimes it doesn't even contain a valid executable name, instead it contains just some string.
/// 3. In case of zombie processes it contains nothing.
///
/// ## /proc/{pid}/exe
/// Then there is `/proc/{pid}/exe`.
/// Which normally is a symlink to the executable of the program.
/// Except that it sometimes isn't. So:
///
/// 1. Different threads may have different symlinks.
/// 2. The symlink might not be available if the main thread exited early e.g. via `pthread_exit()`.
/// 3. It might also point to a deleted file, if the executable got deleted.
pub fn try_find_command_in_proc(pid: i32) -> Result<Vec<OsString>, FindError> {
    let cmdline = std::fs::read(format!("/proc/{pid}/cmdline"))?;

    if cmdline.is_empty() {
        Err(FindError::ProcessIsZombie)
    } else {
        let seperated: Vec<_> = cmdline
            .split(|&b| b == b'\0')
            .map(OsStr::from_bytes)
            .collect();

        if seperated.len() == 1 && seperated[0].as_bytes().contains(&b' ') {
            let mut seperated: Vec<_> = seperated[0]
                .as_bytes()
                .split(|&b| b == b' ')
                .map(|s| OsString::from_vec(s.to_owned()))
                .collect();

            if !Path::new(&seperated[0]).exists() {
                if let Ok(path) = std::fs::read_link(format!("/proc/{pid}/exe")) {
                    seperated[0] = path.into_os_string();
                }
            }

            Ok(seperated)
        } else {
            Ok(seperated.into_iter().map(ToOwned::to_owned).collect())
        }
    }
}

pub fn find_command(meta: &MetaWindow, min_wm_class_sim: f64) -> Result<Exec, FindError> {
    if !meta.gtk_app_id.is_empty() {
        if let Ok(exec) = try_find_command_by_gtk_app_id(&meta.gtk_app_id) {
            return Ok(exec);
        }
    }

    if !meta.sandboxed_app_id.is_empty() {
        if let Ok(exec) = try_find_command_by_sandboxed_app_id(&meta.sandboxed_app_id) {
            return Ok(exec);
        }
    }

    let proc_cmdline = try_find_command_in_proc(meta.pid)?;
    let alt_window_class = Path::new(&proc_cmdline[0])
        .file_name()
        .map(OsStr::to_string_lossy);

    let window_class = match (meta.window_class.as_str(), alt_window_class.as_deref()) {
        ("", None) => None,
        (w_class, None) => Some(WindowClassProvider::Single(w_class)),
        ("", Some(alt_w_class)) => Some(WindowClassProvider::Single(alt_w_class)),
        (w_class, Some(alt_w_class))
            if w_class != alt_w_class
                && strsim::normalized_levenshtein(w_class, alt_w_class) > min_wm_class_sim =>
        {
            Some(WindowClassProvider::WithAlternative(w_class, alt_w_class))
        }
        (w_class, Some(_)) => Some(WindowClassProvider::Single(w_class)),
    };

    if let Some(window_class) = window_class {
        if let Ok(exec) = try_find_command_by_window_class(window_class) {
            return Ok(exec);
        }
    }

    Ok(Exec::CmdLine(proc_cmdline))
}
