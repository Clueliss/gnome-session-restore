pub mod partial_match_similarity;

use super::FindError;
use crate::session::Exec;
use partial_match_similarity::partial_match_similarity;
use std::{
    ffi::{OsStr, OsString},
    os::unix::ffi::{OsStrExt, OsStringExt},
    path::Path,
};

pub type Error = FindError;
pub type Result<T> = std::result::Result<T, Error>;
pub type Confidence = f64;

pub fn try_find_command_by_gtk_app_id(gtk_app_id: &str) -> Result<Exec> {
    let desktop_file_name = format!("{gtk_app_id}.desktop");
    let p = Path::new("/usr/share/applications").join(&desktop_file_name);

    if p.exists() {
        Ok(Exec::DesktopFile(p))
    } else {
        Err(FindError::NoSuitableEntryFound)
    }
}

pub fn try_find_command_by_sandboxed_app_id<L, P>(
    sandboxed_app_id: &str,
    mut desktop_entry_locations: L,
) -> Result<Exec>
where
    L: Iterator<Item = P>,
    P: AsRef<Path>,
{
    let desktop_file_name = format!("{sandboxed_app_id}.desktop");

    let p = desktop_entry_locations.find_map(|p| {
        let p = p.as_ref().join(&desktop_file_name);
        p.exists().then_some(p)
    });

    match p {
        Some(p) => Ok(Exec::DesktopFile(p)),
        None => Err(FindError::NoSuitableEntryFound),
    }
}

fn try_find_desktop_file_fuzzy<S, D, P>(
    search_term: &str,
    similarity_measure: S,
    desktop_files: D,
) -> Result<(Exec, Confidence)>
where
    S: Fn(&str, &str) -> f64,
    D: Iterator<Item = P>,
    P: AsRef<Path>,
{
    let search_term = search_term.to_lowercase();

    let desktop_file = desktop_files
        .map(|path| {
            let filename = path.as_ref().file_stem().unwrap().to_string_lossy().to_lowercase();
            let sim = similarity_measure(&search_term, &filename);

            (path, sim)
        })
        .reduce(max_by_sim);

    match desktop_file {
        Some((path, confidence)) => Ok((Exec::DesktopFile(path.as_ref().to_owned()), confidence)),
        None => Err(FindError::NoSuitableEntryFound),
    }
}

pub fn try_find_command_by_wm_class<D, P>(wm_class: &str, desktop_files: D) -> Result<(Exec, Confidence)>
where
    D: Iterator<Item = P>,
    P: AsRef<Path>,
{
    try_find_desktop_file_fuzzy(wm_class, strsim::normalized_levenshtein, desktop_files)
}

pub fn try_find_command_by_search_term<D, P>(search_term: &str, desktop_files: D) -> Result<(Exec, Confidence)>
where
    D: Iterator<Item = P>,
    P: AsRef<Path>,
{
    try_find_desktop_file_fuzzy(search_term, partial_match_similarity, desktop_files)
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
pub fn try_find_command_in_proc(pid: i32) -> Result<Vec<OsString>> {
    let cmdline = std::fs::read(format!("/proc/{pid}/cmdline"))?;

    if cmdline.is_empty() {
        Err(FindError::ProcessIsZombie)
    } else {
        let seperated: Vec<_> = cmdline
            .split(|&b| b == b'\0')
            .filter(|b| !b.is_empty())
            .map(OsStr::from_bytes)
            .collect();

        if seperated.len() == 1 && seperated[0].as_bytes().contains(&b' ') {
            let mut seperated: Vec<_> = seperated[0]
                .as_bytes()
                .split(|&b| b == b' ')
                .filter(|b| !b.is_empty())
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

fn max_by_sim<T>(acc @ (_, acc_sim): (T, f64), x @ (_, x_sim): (T, f64)) -> (T, f64) {
    if x_sim > acc_sim {
        x
    } else {
        acc
    }
}
