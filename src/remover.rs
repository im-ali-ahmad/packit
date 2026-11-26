use crate::finder::find_raw;
use crate::utils::*;
use std::{
    collections::BTreeSet,
    env, fs,
    io::{self, Write},
    path::{Component, Path, PathBuf},
};
fn is_safe_install_path(path: &Path, safe_roots: &[PathBuf]) -> bool {
    for install_root in safe_roots {
        if !path.is_absolute()
            || !path.starts_with(install_root)
            || path
                .components()
                .any(|component| matches!(component, Component::ParentDir))
        {
            continue;
        }
        let mut current = install_root.to_path_buf();
        if let Ok(metadata) = fs::symlink_metadata(&current) {
            if metadata.file_type().is_symlink() {
                continue;
            }
        }
        let relative = match path.strip_prefix(install_root) {
            Ok(relative) => relative,
            Err(_) => continue,
        };
        let mut components = relative.components().peekable();
        while let Some(component) = components.next() {
            current.push(component);
            if components.peek().is_some() {
                match fs::symlink_metadata(&current) {
                    Ok(metadata) if metadata.file_type().is_symlink() => {
                        return false;
                    }
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(_) => return false,
                    _ => {}
                }
            }
        }
        return true;
    }
    false
}
pub fn remove(package: String) -> i32 {
    let pack = match find_raw(package) {
        Ok(pack) => pack,
        Err(error) => {
            log_error(error);
            return 1;
        }
    };
    println!("Name:         {}", pack.name);
    println!("UID:          {}", pack.uid);
    println!("Version:      {}", pack.version);
    println!(
        "Install Date: {:02}/{:02}/{:04}",
        pack.install_date.0, pack.install_date.1, pack.install_date.2
    );
    for attempt in 1..=2 {
        print!("Confirmation ({}/2) to remove [yes]: ", attempt);
        let _ = io::stdout().flush();
        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(_) => {
                if !input.trim().eq_ignore_ascii_case("yes") {
                    log_warning("Removal cancelled.");
                    return 1;
                }
            }
            Err(e) => {
                log_error(format!("Cannot read confirmation: {}", e));
                return 1;
            }
        }
    }
    let home = match env::var_os("HOME") {
        Some(home) => PathBuf::from(home),
        None => {
            log_error("Cannot determine home directory.");
            return 1;
        }
    };
    let uid = Path::new(&pack.uid);
    if uid.components().count() != 1
        || !matches!(uid.components().next(), Some(Component::Normal(_)))
    {
        log_error("Invalid package UID.");
        return 1;
    }
    let install_root = home.join(".local/packit.opt").join(uid);
    let safe_roots = [install_root.clone(), home.join(".local/bin")];
    log_step(format!("Removing package '{}'...", pack.uid));
    let mut success = true;
    let mut directories = BTreeSet::new();
    for filename in &pack.files {
        let path = Path::new(filename);
        if !is_safe_install_path(path, &safe_roots) {
            log_warning(format!("Skipping invalid installation path: {}", filename));
            success = false;
            continue;
        }
        if let Some(parent) = path.parent() {
            let mut current = parent.to_path_buf();
            while current.starts_with(&install_root) {
                directories.insert(current.clone());
                if current == install_root || !current.pop() {
                    break;
                }
            }
        }
        match fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                log_warning(format!("Already missing: {}", filename));
            }
            Err(error) => {
                log_error(format!("Cannot remove '{}': {}", filename, error));
                success = false;
            }
        }
    }
    for directory in directories.iter().rev() {
        match fs::remove_dir(directory) {
            Ok(()) => {}
            Err(error)
                if error.kind() == io::ErrorKind::DirectoryNotEmpty
                    || error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                log_error(format!(
                    "Cannot remove directory '{}': {}",
                    directory.display(),
                    error
                ));
                success = false;
            }
        }
    }
    if success {
        log_success(format!("Package '{}' removed successfully.", pack.uid));
        0
    } else {
        log_warning(format!(
            "Package '{}' was only partially removed.",
            pack.uid
        ));
        1
    }
}
