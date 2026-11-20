use crate::installed_manifest::Package;
use std::{fs, path::PathBuf};
const INSTALL_DIR: &str = ".local/packit.opt";
const INSTALL_MANIFEST: &str = "packit-manifest.toml";
pub fn find_all() {
    let home = match std::env::var_os("HOME") {
        Some(home) => PathBuf::from(home),
        None => {
            println!("Cannot determine HOME directory.");
            return;
        }
    };
    let install_root = home.join(INSTALL_DIR);
    let entries = match fs::read_dir(&install_root) {
        Ok(entries) => entries,
        Err(e) => {
            println!("Cannot access installed packages: {}", e);
            return;
        }
    };
    let mut packages = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                println!("Cannot read directory entry: {}", e);
                continue;
            }
        };
        let metadata = match fs::symlink_metadata(entry.path()) {
            Ok(metadata) => metadata,
            Err(e) => {
                println!("Cannot inspect '{}': {}", entry.path().display(), e);
                continue;
            }
        };
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            continue;
        }
        let manifest_path = entry.path().join(INSTALL_MANIFEST);
        let content = match fs::read_to_string(&manifest_path) {
            Ok(content) => content,
            Err(e) => {
                println!("Cannot read manifest '{}': {}", manifest_path.display(), e);
                continue;
            }
        };
        let package: Package = match toml::from_str(&content) {
            Ok(package) => package,
            Err(e) => {
                println!("Invalid manifest '{}': {}", manifest_path.display(), e);
                continue;
            }
        };
        if package.uid != entry.file_name().to_string_lossy() {
            println!("UID mismatch in '{}'. Skipping.", manifest_path.display());
            continue;
        }
        packages.push(package);
    }
    packages.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    if packages.is_empty() {
        println!("No packages installed.");
        return;
    }
    for package in packages {
        println!(
            "{} {}-{} [{:02}/{:02}/{:04}]",
            package.name,
            package.uid,
            package.version,
            package.install_date.0,
            package.install_date.1,
            package.install_date.2
        );
    }
}
pub fn find_raw(package: String) -> Result<Package, String> {
    if package.is_empty()
        || !package
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        || package == "."
        || package == ".."
    {
        return Err("Invalid package UID.".to_string());
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| "Cannot determine HOME directory.".to_string())?;
    let install_root = home.join(INSTALL_DIR);
    let root_metadata = fs::symlink_metadata(&install_root)
        .map_err(|e| format!("Cannot access installed packages: {}", e))?;
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return Err("Invalid installation directory.".to_string());
    }
    let package_dir = install_root.join(&package);
    let package_metadata = fs::symlink_metadata(&package_dir)
        .map_err(|_| format!("No such package found: '{}'.", package))?;
    if package_metadata.file_type().is_symlink() || !package_metadata.is_dir() {
        return Err(format!("Invalid package directory '{}'.", package));
    }
    let manifest_path = package_dir.join(INSTALL_MANIFEST);
    let manifest_metadata = fs::symlink_metadata(&manifest_path)
        .map_err(|_| format!("Package manifest '{}' was not found.", package))?;
    if manifest_metadata.file_type().is_symlink() || !manifest_metadata.is_file() {
        return Err("Invalid package manifest.".to_string());
    }
    let content = fs::read_to_string(&manifest_path)
        .map_err(|e| format!("Cannot read package manifest: {}", e))?;
    let item: Package =
        toml::from_str(&content).map_err(|e| format!("Invalid package manifest: {}", e))?;
    if item.uid != package {
        return Err("Package UID does not match its installation directory.".to_string());
    }
    Ok(item)
}
pub fn find(package: String) -> i32 {
    match find_raw(package) {
        Ok(item) => {
            println!("Name:         {}", item.name);
            println!("UID:          {}", item.uid);
            println!("Version:      {}", item.version);
            println!("Description:  {}", item.description);
            println!("License:      {}", item.license);
            println!("Authors:      {}", item.authors.join(", "));
            println!("Files:        {} installed", item.files.len());
            println!(
                "Install Date: {:02}/{:02}/{:04} DD/MM/YYYY",
                item.install_date.0, item.install_date.1, item.install_date.2
            );
            0
        }
        Err(msg) => {
            println!("{}", msg);
            1
        }
    }
}
