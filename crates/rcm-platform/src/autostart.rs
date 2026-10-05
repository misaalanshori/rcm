use rcm_core::CoreError;

const AUTOSTART_NAME: &str = "RcloneManagerAgent";

/// Enables autostart at user login (DM-6, §7.10)
pub fn enable_autostart(app_path: &str, args: &[&str]) -> Result<(), CoreError> {
    #[cfg(windows)]
    {
        let mut full_cmd = format!("\"{}\"", app_path);
        for arg in args {
            full_cmd.push_str(&format!(" {}", arg));
        }

        let status = std::process::Command::new("reg")
            .args([
                "add",
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                "/v",
                AUTOSTART_NAME,
                "/t",
                "REG_SZ",
                "/d",
                &full_cmd,
                "/f",
            ])
            .status()
            .map_err(|e| CoreError::Validation(format!("Failed to execute reg add: {}", e)))?;

        if !status.success() {
            return Err(CoreError::Validation("Failed to set autostart registry entry".to_string()));
        }
        Ok(())
    }

    #[cfg(not(windows))]
    {
        let home = std::env::var("HOME").map_err(|_| CoreError::Validation("HOME not set".to_string()))?;
        let autostart_dir = std::path::Path::new(&home).join(".config").join("autostart");
        let _ = std::fs::create_dir_all(&autostart_dir);
        let desktop_file = autostart_dir.join("rcm-agent.desktop");

        let mut exec = app_path.to_string();
        for arg in args {
            exec.push_str(&format!(" {}", arg));
        }

        let content = format!(
            "[Desktop Entry]\nType=Application\nName=Rclone Manager Agent\nExec={}\nTerminal=false\nNoDisplay=true\n",
            exec
        );

        std::fs::write(desktop_file, content)
            .map_err(|e| CoreError::Validation(format!("Failed to write autostart desktop file: {}", e)))?;
        Ok(())
    }
}

/// Disables autostart at user login (DM-6)
pub fn disable_autostart() -> Result<(), CoreError> {
    #[cfg(windows)]
    {
        let status = std::process::Command::new("reg")
            .args([
                "delete",
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                "/v",
                AUTOSTART_NAME,
                "/f",
            ])
            .status()
            .map_err(|e| CoreError::Validation(format!("Failed to execute reg delete: {}", e)))?;

        // Exit status 0 is success; if key didn't exist it might return 1, which is fine
        let _ = status;
        Ok(())
    }

    #[cfg(not(windows))]
    {
        let home = std::env::var("HOME").map_err(|_| CoreError::Validation("HOME not set".to_string()))?;
        let desktop_file = std::path::Path::new(&home)
            .join(".config")
            .join("autostart")
            .join("rcm-agent.desktop");
        if desktop_file.exists() {
            let _ = std::fs::remove_file(desktop_file);
        }
        Ok(())
    }
}

/// Checks whether autostart is currently enabled
pub fn is_autostart_enabled() -> bool {
    #[cfg(windows)]
    {
        if let Ok(output) = std::process::Command::new("reg")
            .args([
                "query",
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                "/v",
                AUTOSTART_NAME,
            ])
            .output()
        {
            output.status.success()
        } else {
            false
        }
    }

    #[cfg(not(windows))]
    {
        if let Ok(home) = std::env::var("HOME") {
            let desktop_file = std::path::Path::new(&home)
                .join(".config")
                .join("autostart")
                .join("rcm-agent.desktop");
            desktop_file.exists()
        } else {
            false
        }
    }
}
