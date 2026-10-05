use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeTokens {
    pub is_dark: bool,
    pub background: String,
    pub surface: String,
    pub surface_hover: String,
    pub primary: String,
    pub text: String,
    pub muted_text: String,
    pub border: String,
    pub success: String,
    pub warning: String,
    pub danger: String,
}

impl ThemeTokens {
    pub fn dark() -> Self {
        Self {
            is_dark: true,
            background: "#181825".to_string(),
            surface: "#1e1e2e".to_string(),
            surface_hover: "#313244".to_string(),
            primary: "#89b4fa".to_string(),
            text: "#cdd6f4".to_string(),
            muted_text: "#a6adc8".to_string(),
            border: "#45475a".to_string(),
            success: "#a6e3a1".to_string(),
            warning: "#f9e2af".to_string(),
            danger: "#f38ba8".to_string(),
        }
    }

    pub fn light() -> Self {
        Self {
            is_dark: false,
            background: "#eff1f5".to_string(),
            surface: "#ffffff".to_string(),
            surface_hover: "#e6e9ef".to_string(),
            primary: "#1e66f5".to_string(),
            text: "#4c4f69".to_string(),
            muted_text: "#7c7f93".to_string(),
            border: "#ccd0da".to_string(),
            success: "#40a02b".to_string(),
            warning: "#df8e1d".to_string(),
            danger: "#d20f39".to_string(),
        }
    }
}

impl Default for ThemeTokens {
    fn default() -> Self {
        Self::dark()
    }
}
