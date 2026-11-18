pub enum InitializeType {
    PROJECTLOCAL,
    TARLOCAL,
    TARREMOTE,
}

pub fn initialize(init_type: InitializeType, url: String) -> i32 {
    match init_type {
        InitializeType::PROJECTLOCAL => {}
        InitializeType::TARLOCAL => {}
        InitializeType::TARREMOTE => {}
    }

    return 0;
}
