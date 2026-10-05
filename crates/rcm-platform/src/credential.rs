use keyring::Entry;
use rcm_core::CoreError;

const SERVICE_NAME: &str = "rclone-manager";

/// Stores a credential/password securely in the OS keyring (CF-8, §7.12)
pub fn set_credential(key: &str, secret: &str) -> Result<(), CoreError> {
    let entry = Entry::new(SERVICE_NAME, key)
        .map_err(|e| CoreError::Validation(format!("Failed to initialize keyring entry: {}", e)))?;
    entry
        .set_password(secret)
        .map_err(|e| CoreError::Validation(format!("Failed to store secret in keyring: {}", e)))?;
    Ok(())
}

/// Retrieves a credential/password securely from the OS keyring (CF-8, §7.12)
pub fn get_credential(key: &str) -> Result<String, CoreError> {
    let entry = Entry::new(SERVICE_NAME, key)
        .map_err(|e| CoreError::Validation(format!("Failed to initialize keyring entry: {}", e)))?;
    entry
        .get_password()
        .map_err(|e| CoreError::NotFound(format!("Keyring secret '{}' not found: {}", key, e)))
}

/// Deletes a credential from the OS keyring
pub fn delete_credential(key: &str) -> Result<(), CoreError> {
    let entry = Entry::new(SERVICE_NAME, key)
        .map_err(|e| CoreError::Validation(format!("Failed to initialize keyring entry: {}", e)))?;
    let _ = entry.delete_credential();
    Ok(())
}
