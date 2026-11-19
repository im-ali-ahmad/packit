use crate::{installer_manifest, utils::hash_directory};
use flate2::{Compression, read::GzDecoder, write::GzEncoder};
use reqwest::blocking::Response;
use serde::Deserialize;
use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};
use tar::{Archive, Builder};
use tempfile::TempDir;

#[derive(Debug, Deserialize, Clone)]
struct Manifest {
    r#type: String,
    path: String,
    files: Option<Files>,
    commands: Vec<Command>,
    project: Project,
}

#[derive(Debug, Deserialize, Clone)]
struct Files {
    base: String,
    extra: Vec<ExtraFile>,
}

#[derive(Debug, Deserialize, Clone)]
struct ExtraFile {
    source: String,
    dest: String,
}

#[derive(Debug, Deserialize, Clone)]
struct Command {
    file: String,
    name: String,
}

#[derive(Debug, Deserialize, Clone)]
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

fn pack_folder(src: &Path, manifest: &Manifest, out_path: &PathBuf) -> i32 {
    let mut success: bool = true;
    let manifest_file: PathBuf = out_path.join("packit-manifest.toml");

    let mut commands: Vec<installer_manifest::Command> = Vec::new();
    for command in &manifest.commands {
        let command_path: PathBuf = src.join(&command.file);
        if !command_path.exists() {
            println!(
                "Command file '{}' does not exist.",
                command_path.to_string_lossy()
            );

            success = false;
            break;
        }

        commands.push(installer_manifest::Command {
            file: command.file.clone(),
            name: command.name.clone(),
        });
    }

    let install_manifest = installer_manifest::Manifest {
        commands,
        project: installer_manifest::Project {
            uid: manifest.project.uid.clone(),
            name: manifest.project.name.clone(),
            description: manifest.project.description.clone(),
            author: manifest.project.author.clone(),
            version: manifest.project.version.clone(),
            license: manifest.project.license.clone(),
        },
        hash: if let Ok(h) = hash_directory(src) {
            h
        } else {
            println!("Cannot compute hash of the folder.");
            success = false;
            String::new()
        },
    };

    let serialized = match toml::to_string(&install_manifest) {
        Ok(data) => data,
        Err(e) => {
            println!("Cannot serialize manifest: {}", e);
            success = false;
            String::new()
        }
    };

    if success {
        if let Err(e) =
            File::create(&manifest_file).and_then(|mut file| file.write_all(serialized.as_bytes()))
        {
            println!("Cannot write manifest file: {}", e);
            success = false;
        }
    }

    if !success {
        return 1;
    }

    let pack_path: File = if let Ok(p) = File::create(format!(
        "{}-{}.pack",
        manifest.project.uid, manifest.project.version
    )) {
        p
    } else {
        println!("Cannot create output file.");
        return 1;
    };

    let encoder: GzEncoder<File> = GzEncoder::new(pack_path, Compression::default());
    let archive: Builder<GzEncoder<File>> = Builder::new(encoder);
    let encoder: GzEncoder<File> = if let Ok(e) = archive.into_inner() {
        e
    } else {
        println!("Cannot create archive.");
        return 1;
    };

    match encoder.finish() {
        Ok(_) => {}
        Err(_) => {
            println!("Cannot finish archive.");
            return 1;
        }
    };

    println!(
        "Project packed successfully. Output file: '{}-{}.pack'",
        manifest.project.uid, manifest.project.version
    );
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

        return pack_folder(&out_path, manifest, &out_path);
    }

    println!("No files specified in the manifest.");
    return 1;
}

fn build_tar_local(manifest: &Manifest) -> i32 {
    println!("Extracting local archive: '{}'", manifest.path);

    let archive_file: File = match File::open(&manifest.path) {
        Ok(file) => file,
        Err(e) => {
            println!("Cannot open archive: {}", e);
            return 1;
        }
    };

    let temp_dir: TempDir = match tempfile::tempdir() {
        Ok(dir) => dir,
        Err(e) => {
            println!("Cannot create temporary directory: {}", e);
            return 1;
        }
    };

    let decoder: GzDecoder<File> = GzDecoder::new(archive_file);
    let mut archive: Archive<GzDecoder<File>> = Archive::new(decoder);

    if let Err(e) = archive.unpack(temp_dir.path()) {
        println!("Cannot extract archive: {}", e);
        return 1;
    }

    println!("Archive extracted successfully.");
    let mut local_manifest: Manifest = manifest.clone();

    local_manifest.r#type = "PROJECTLOCAL".to_string();
    local_manifest.path = ".".to_string();

    let cwd: String = temp_dir.path().to_string_lossy().into_owned();
    let result: i32 = build_project_local(&local_manifest, cwd);

    if result != 0 {
        println!("Local archive build failed.");
        return result;
    }

    println!("Local archive build completed successfully.");
    return 0;
}

fn build_tar_remote(manifest: &Manifest) -> i32 {
    let url: &String = &manifest.path;
    println!("Downloading remote archive: '{}'", url);

    let response: Response = match reqwest::blocking::get(url) {
        Ok(response) => response,
        Err(e) => {
            println!("Cannot download remote archive: {}", e);
            return 1;
        }
    };

    if !response.status().is_success() {
        println!("Remote server returned HTTP status: {}", response.status());
        return 1;
    }

    let temp_dir: TempDir = match tempfile::tempdir() {
        Ok(dir) => dir,
        Err(e) => {
            println!("Cannot create temporary directory: {}", e);
            return 1;
        }
    };

    let archive_path: PathBuf = temp_dir.path().join("remote.tar.gz");
    let mut archive_file: File = match File::create(&archive_path) {
        Ok(file) => file,
        Err(e) => {
            println!("Cannot create archive file: {}", e);
            return 1;
        }
    };

    let mut response: Response = response;
    if let Err(e) = std::io::copy(&mut response, &mut archive_file) {
        println!("Cannot save downloaded archive: {}", e);
        return 1;
    }

    drop(archive_file);
    println!("Remote archive downloaded successfully.");

    let mut local_manifest: Manifest = manifest.clone();

    local_manifest.r#type = "TARLOCAL".to_string();
    local_manifest.path = archive_path.to_string_lossy().into_owned();

    let result: i32 = build_tar_local(&local_manifest);

    if result != 0 {
        println!("Remote archive build failed.");
        return result;
    }

    println!("Remote archive build completed successfully.");
    return 0;
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
