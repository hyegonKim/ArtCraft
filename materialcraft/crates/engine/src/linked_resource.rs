/// Read resource paths relative to an SBS package, including portable library aliases.
pub fn reader(path: &str) -> impl Fn(&str) -> Option<Vec<u8>> + use<> {
    let owner = path.to_string();
    #[cfg(not(target_arch = "wasm32"))]
    let folder = {
        let parent = std::path::Path::new(path).parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or(std::path::Path::new("."));
        std::fs::canonicalize(parent).ok()
    };

    move |reference: &str| {
        if reference.starts_with("sbs://") {
            return materialcraft_formats::sbs::library::read(reference).ok();
        }

        if owner.starts_with("sbs://") {
            let uri = materialcraft_formats::sbs::library::resolve(&owner, reference).ok()?;
            return materialcraft_formats::sbs::library::read(&uri).ok();
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let relative = reference.trim().replace('\\', "/");
            let plain = !relative.is_empty() && !relative.contains(':') && !relative.starts_with('/');
            if !plain || std::path::Path::new(&relative).components().any(|part| !matches!(part, std::path::Component::Normal(_))) {
                return None;
            }

            let folder = folder.as_ref()?;
            let file = std::fs::canonicalize(folder.join(&relative)).ok()?;
            let metadata = std::fs::metadata(&file).ok()?;
            if !file.starts_with(folder) || !metadata.is_file() {
                return None;
            }

            std::fs::read(file).ok()
        }
        #[cfg(target_arch = "wasm32")]
        None
    }
}
