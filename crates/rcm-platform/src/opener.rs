use rcm_core::CoreError;

pub fn open_path_in_file_manager(path: &str) -> Result<(), CoreError> {
    open::that(path).map_err(|e| {
        CoreError::Validation(format!("Failed to open path '{}' in file manager: {}", path, e))
    })
}

pub fn open_url_in_browser(url: &str) -> Result<(), CoreError> {
    open::that(url).map_err(|e| {
        CoreError::Validation(format!("Failed to open URL '{}' in browser: {}", url, e))
    })
}
