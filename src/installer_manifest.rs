use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub commands: Vec<Command>,
    pub project: Project,
    pub hash: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Command {
    pub file: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Project {
    pub uid: String,
    pub name: String,
    pub description: String,
    pub author: Vec<String>,
    pub version: String,
    pub license: String,
}
