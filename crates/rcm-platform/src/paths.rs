use camino::Utf8PathBuf;
use directories::BaseDirs;
use rcm_core::CoreError;

#[derive(Debug, Clone)]
pub struct RcmPaths {
    config_dir: Utf8PathBuf,
    data_dir: Utf8PathBuf,
    backups_dir: Utf8PathBuf,
    logs_dir: Utf8PathBuf,
    rclone_versions_dir: Utf8PathBuf,
    state_dir: Utf8PathBuf,
    default_rclone_config_path: Utf8PathBuf,
}

impl RcmPaths {
    pub fn resolve() -> Result<Self, CoreError> {
        let base_dirs = BaseDirs::new()
            .ok_or_else(|| CoreError::Validation("Failed to resolve base system directories".to_string()))?;

        #[cfg(windows)]
        let (config_dir, data_dir, state_dir) = {
            let app_data = Utf8PathBuf::from_path_buf(base_dirs.config_dir().to_path_buf())
                .map_err(|_| CoreError::Validation("Invalid UTF-8 path in config dir".to_string()))?;
            let local_app_data = Utf8PathBuf::from_path_buf(base_dirs.data_local_dir().to_path_buf())
                .map_err(|_| CoreError::Validation("Invalid UTF-8 path in local data dir".to_string()))?;
            (
                app_data.join("RCM"),
                local_app_data.join("RCM"),
                local_app_data.join("RCM").join("state"),
            )
        };

        #[cfg(not(windows))]
        let (config_dir, data_dir, state_dir) = {
            let home = Utf8PathBuf::from_path_buf(base_dirs.home_dir().to_path_buf())
                .map_err(|_| CoreError::Validation("Invalid UTF-8 path in home dir".to_string()))?;
            (
                home.join(".config").join("rcm"),
                home.join(".local").join("share").join("rcm"),
                home.join(".local").join("state").join("rcm"),
            )
        };

        let backups_dir = data_dir.join("backups");
        let logs_dir = data_dir.join("logs");
        let rclone_versions_dir = data_dir.join("rclone");

        #[cfg(windows)]
        let default_rclone_config_path = {
            let app_data = Utf8PathBuf::from_path_buf(base_dirs.config_dir().to_path_buf())
                .map_err(|_| CoreError::Validation("Invalid UTF-8 path in config dir".to_string()))?;
            app_data.join("rclone").join("rclone.conf")
        };

        #[cfg(not(windows))]
        let default_rclone_config_path = {
            let home = Utf8PathBuf::from_path_buf(base_dirs.home_dir().to_path_buf())
                .map_err(|_| CoreError::Validation("Invalid UTF-8 path in home dir".to_string()))?;
            home.join(".config").join("rclone").join("rclone.conf")
        };

        Ok(Self {
            config_dir,
            data_dir,
            backups_dir,
            logs_dir,
            rclone_versions_dir,
            state_dir,
            default_rclone_config_path,
        })
    }

    pub fn config_dir(&self) -> &Utf8PathBuf {
        &self.config_dir
    }

    pub fn data_dir(&self) -> &Utf8PathBuf {
        &self.data_dir
    }

    pub fn backups_dir(&self) -> &Utf8PathBuf {
        &self.backups_dir
    }

    pub fn logs_dir(&self) -> &Utf8PathBuf {
        &self.logs_dir
    }

    pub fn rclone_versions_dir(&self) -> &Utf8PathBuf {
        &self.rclone_versions_dir
    }

    pub fn state_dir(&self) -> &Utf8PathBuf {
        &self.state_dir
    }

    pub fn default_rclone_config_path(&self) -> &Utf8PathBuf {
        &self.default_rclone_config_path
    }

    pub fn profiles_file_path(&self) -> Utf8PathBuf {
        self.config_dir.join("profiles.toml")
    }

    pub fn settings_file_path(&self) -> Utf8PathBuf {
        self.config_dir.join("settings.toml")
    }

    pub fn ensure_dirs(&self) -> Result<(), CoreError> {
        let dirs = [
            &self.config_dir,
            &self.data_dir,
            &self.backups_dir,
            &self.logs_dir,
            &self.rclone_versions_dir,
            &self.state_dir,
        ];
        for d in dirs {
            std::fs::create_dir_all(d).map_err(|e| {
                CoreError::Validation(format!("Failed to create directory '{}': {}", d, e))
            })?;
        }
        Ok(())
    }
}
