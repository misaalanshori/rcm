use std::collections::BTreeMap;
use rcm_core::CoreError;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IniSection {
    pub name: String,
    pub properties: BTreeMap<String, String>,
}

impl IniSection {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            properties: BTreeMap::new(),
        }
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.properties.get(key).map(|s| s.as_str())
    }

    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.properties.insert(key.into(), value.into());
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IniConfig {
    pub sections: BTreeMap<String, IniSection>,
    pub section_order: Vec<String>,
}

impl IniConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn section(&self, name: &str) -> Option<&IniSection> {
        self.sections.get(name)
    }

    pub fn section_mut(&mut self, name: &str) -> Option<&mut IniSection> {
        self.sections.get_mut(name)
    }

    pub fn remote_names(&self) -> Vec<String> {
        self.section_order.clone()
    }

    /// Read-only parser for rclone.conf (CI-1, §7.5)
    pub fn parse_str(content: &str) -> Result<Self, CoreError> {
        let mut config = Self::new();
        let mut current_section: Option<IniSection> = None;

        for (line_idx, raw_line) in content.lines().enumerate() {
            let line = raw_line.trim();
            // Skip empty lines and comment lines
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }

            if line.starts_with('[') {
                if let Some(end_bracket) = line.find(']') {
                    let section_name = line[1..end_bracket].trim();
                    if section_name.is_empty() {
                        return Err(CoreError::Validation(format!(
                            "Line {}: Empty section header",
                            line_idx + 1
                        )));
                    }

                    if let Some(prev) = current_section.take() {
                        config.section_order.push(prev.name.clone());
                        config.sections.insert(prev.name.clone(), prev);
                    }

                    current_section = Some(IniSection::new(section_name));
                    continue;
                } else {
                    return Err(CoreError::Validation(format!(
                        "Line {}: Unclosed section header",
                        line_idx + 1
                    )));
                }
            }

            // Key = Value
            if let Some(ref mut sec) = current_section {
                if let Some(equals_pos) = line.find('=') {
                    let key = line[..equals_pos].trim();
                    let val = line[equals_pos + 1..].trim();
                    if !key.is_empty() {
                        sec.insert(key, val);
                    }
                }
            } else {
                // Key-value pair found before any section header
                return Err(CoreError::Validation(format!(
                    "Line {}: Property defined outside of a section",
                    line_idx + 1
                )));
            }
        }

        if let Some(prev) = current_section.take() {
            config.section_order.push(prev.name.clone());
            config.sections.insert(prev.name.clone(), prev);
        }

        Ok(config)
    }
}
