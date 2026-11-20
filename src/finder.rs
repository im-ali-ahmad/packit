use crate::installed_manifest::Package;

pub fn find_all() {}

pub fn find_raw(package: String) -> Result<Package, String> {
    Err("No such package found.".to_string())
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
                "Install Date: {:02}/{:02}/{:4} DD/MM/YYYY",
                item.install_date.0, item.install_date.1, item.install_date.2
            );
            return 0;
        }

        Err(msg) => {
            println!("{}", msg);
            return 1;
        }
    }
}
