use crate::finder::find_raw;
use crate::utils::log;
use std::{
    collections::BTreeSet,
    env, fs, io,
    path::{Component, Path, PathBuf},
};
pub fn remove(package: String) -> i32 {
    let pack = match find_raw(package) {
        Ok(pack) => pack,
        Err(err) => {
            log(format!("Error: {}", err));
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
    let stdin = io::stdin();
    for attempt in 1..=2 {
        log(format!(
            "Confirm removal ({}/2). Type 'yes' to continue:",
            attempt
        ));
        let mut input = String::new();
        if let Err(e) = stdin.read_line(&mut input) {
            log(format!("Cannot read confirmation: {}", e));
            return 1;
        }
        if !input.trim().eq_ignore_ascii_case("yes") {
            log("Removal cancelled.");
            return 1;
        }
    }
    let home = match env::var_os("HOME") {
        Some(home) => PathBuf::from(home),
        None => {
            log("Cannot determine home directory.");
            return 1;
        }
    };
    let install_root = home.join(".local/packit.opt").join(&pack.uid);
    log(format!("Removing package '{}'...", pack.uid));
    let mut success = true;
    let mut directories = BTreeSet::new();
    for filename in &pack.files {
        let path = Path::new(filename);
        if !path.is_absolute()
            || !path.starts_with(&install_root)
            || path.components().any(|c| matches!(c, Component::ParentDir))
        {
            log(format!("Skipping invalid installation path: {}", filename));
            success = false;
            continue;
        }
        if let Some(parent) = path.parent() {
            let mut current = parent.to_path_buf();
            while current.starts_with(&install_root) {
                directories.insert(current.clone());
                if current == install_root {
                    break;
                }
                if !current.pop() {
                    break;
                }
            }
        }
        match fs::remove_file(path) {
            Ok(()) => {
                log(format!("Removed: {}", filename));
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                log(format!("Already missing: {}", filename));
            }
            Err(e) => {
                log(format!("Cannot remove '{}': {}", filename, e));
                success = false;
            }
        }
    }
    for directory in directories.iter().rev() {
        match fs::remove_dir(directory) {
            Ok(()) => {
                log(format!("Removed empty directory: {}", directory.display()));
            }
            Err(e) if e.kind() == io::ErrorKind::DirectoryNotEmpty => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => {
                log(format!(
                    "Cannot remove directory '{}': {}",
                    directory.display(),
                    e
                ));
                success = false;
            }
        }
    }
    if success {
        log(format!("Package '{}' removed successfully.", pack.uid));
        return 0;
    } else {
        log(format!(
            "Package '{}' was only partially removed.",
            pack.uid
        ));
        return 1;
    }
}
