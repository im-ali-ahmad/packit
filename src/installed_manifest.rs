use serde::{Deserialize, Serialize};
#[derive(Debug, Serialize, Deserialize)]
pub struct Package {
    pub uid: String,
    pub name: String,
    pub description: String,
    pub authors: Vec<String>,
    pub version: String,
    pub license: String,
    pub files: Vec<String>,
    pub install_date: (u8, u8, u16),
}
