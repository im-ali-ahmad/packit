use serde::Deserialize;
use std::fs;

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

fn build_project_local(manifest: Manifest) -> i32 {
    0
}

fn build_tar_local(manifest: Manifest) -> i32 {
    0
}

fn build_tar_remote(manifest: Manifest) -> i32 {
    0
}

pub fn build(path: String) -> i32 {
    match parse_manifest(&path) {
        Ok(manifest) => {
            if manifest.r#type == "PROJECTlOCAL" {
                return build_project_local(manifest);
            }

            if manifest.r#type == "TARLOCAL" {
                return build_tar_local(manifest);
            }

            if manifest.r#type == "TARREMOTE" {
                return build_tar_remote(manifest);
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
