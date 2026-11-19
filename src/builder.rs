use serde::Deserialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Deserialize)]
struct Manifest {
    r#type: String,
    path: String,
    files: Option<Files>,
    commands: Vec<Command>,
    project: Project,
}

#[derive(Debug, Deserialize)]
struct Files {
    base: String,
    extra: Vec<ExtraFile>,
}

#[derive(Debug, Deserialize)]
struct ExtraFile {
    source: String,
    dest: String,
}

#[derive(Debug, Deserialize)]
struct Command {
    file: String,
    name: String,
}

#[derive(Debug, Deserialize)]
struct Project {
    uid: String,
    name: String,
    description: String,
    author: Vec<String>,
    version: String,
    license: String,
}

fn parse_manifest(path: &str) -> Result<Manifest, String> {
    let content: String = fs::read_to_string(path).map_err(|e| e.to_string())?;
    toml::from_str::<Manifest>(&content).map_err(|e| e.to_string())
}

fn copy_dir_content(src: &Path, dest: &Path) -> Result<(), String> {
    if !src.is_dir() {
        return Err(format!(
            "Source path '{}' is not a directory.",
            src.display()
        ));
    }

    fs::create_dir_all(dest).map_err(|e| e.to_string())?;

    for entry in fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let file_type = entry.file_type().map_err(|e| e.to_string())?;
        let dest_path = dest.join(entry.file_name());

        if file_type.is_dir() {
            copy_dir_content(&entry.path(), &dest_path)?;
        } else if file_type.is_file() {
            fs::copy(&entry.path(), &dest_path).map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

fn pack_folder(src: &Path, manifest: &Manifest) -> i32 {
    return 0;
}

fn build_project_local(manifest: &Manifest, cwd: String) -> i32 {
    if let Some(files) = &manifest.files {
        let project_path: PathBuf = Path::new(&cwd).join(&manifest.path);
        if !project_path.is_dir() {
            println!(
                "Project folder '{}' does not exist.",
                project_path.to_string_lossy()
            );

            return 1;
        }

        let base_path: PathBuf = project_path.join(&files.base);
        if !base_path.is_dir() {
            println!(
                "Base folder '{}' does not exist.",
                base_path.to_string_lossy()
            );

            return 1;
        }

        let mut success: bool = true;
        let out_path: PathBuf = project_path.join("packit.out");

        if out_path.exists() {
            let _ = out_path
                .canonicalize()
                .map_err(|e| e.to_string())
                .map(|p| fs::remove_dir_all(p).map_err(|e| e.to_string()))
                .unwrap_or_else(|e| {
                    println!("Cannot remove existing output folder: {}", e);
                    success = false;
                    Ok(())
                });

            if !success {
                return 1;
            }
        }

        fs::create_dir_all(&out_path)
            .map_err(|e| e.to_string())
            .unwrap_or_else(|_| {
                println!(
                    "Cannot create output folder '{}'.",
                    out_path.to_string_lossy()
                );
                success = false;
            });

        if !success {
            return 1;
        }

        match copy_dir_content(&base_path, &out_path) {
            Ok(_) => {}
            Err(e) => {
                println!("Cannot copy base folder: {}", e);
                return 1;
            }
        }

        for file in &files.extra {
            let source_path: PathBuf = project_path.join(&file.source);
            let dest_path: PathBuf = out_path.join(&file.dest);

            if !source_path.exists() {
                println!(
                    "Extra file/folder '{}' does not exist.",
                    source_path.to_string_lossy()
                );
                success = false;
                continue;
            }

            if source_path.is_dir() {
                match copy_dir_content(&source_path, &dest_path) {
                    Ok(_) => {}
                    Err(e) => {
                        println!("Cannot copy extra folder: {}", e);
                        success = false;
                    }
                }
            } else if source_path.is_file() {
                fs::create_dir_all(dest_path.parent().unwrap())
                    .map_err(|e| e.to_string())
                    .unwrap_or_else(|_| {
                        println!(
                            "Cannot create destination folder '{}'.",
                            dest_path.parent().unwrap().to_string_lossy()
                        );
                        success = false;
                    });

                if !success {
                    continue;
                }

                fs::copy(&source_path, &dest_path)
                    .map_err(|e| e.to_string())
                    .unwrap_or_else(|_| {
                        println!(
                            "Cannot copy extra file '{}' to '{}'.",
                            source_path.to_string_lossy(),
                            dest_path.to_string_lossy()
                        );
                        success = false;
                        0
                    });
            }

            if !success {
                return 1;
            }
        }

        println!(
            "Project local build completed successfully. Output folder: '{}'",
            out_path.to_string_lossy()
        );

        return pack_folder(&out_path, manifest);
    }

    println!("No files specified in the manifest.");
    return 1;
}

fn build_tar_local(manifest: &Manifest) -> i32 {
    0
}

fn build_tar_remote(manifest: &Manifest) -> i32 {
    0
}

pub fn build(path: String) -> i32 {
    match parse_manifest(&path) {
        Ok(manifest) => {
            if manifest.r#type == "PROJECTLOCAL" {
                return build_project_local(
                    &manifest,
                    std::env::current_dir()
                        .unwrap()
                        .to_string_lossy()
                        .into_owned(),
                );
            }

            if manifest.r#type == "TARLOCAL" {
                return build_tar_local(&manifest);
            }

            if manifest.r#type == "TARREMOTE" {
                return build_tar_remote(&manifest);
            }

            println!("Invalid package type.");
            return 1;
        }

        Err(err) => {
            println!("Cannot parse manifest: {}", err);
            return 1;
        }
    }
}
