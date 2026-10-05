use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};
use rcm_rc::types::OptionSchema;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FormFieldKind {
    Toggle {
        default: bool,
    },
    Select {
        options: Vec<(String, String)>, // (value, help)
        default: Option<String>,
    },
    ComboBox {
        suggestions: Vec<String>,
        default: Option<String>,
    },
    NumberWithUnit {
        unit_type: String, // "Duration", "SizeSuffix", "BandwidthSpec", "int"
        default: Option<String>,
    },
    Secret {
        default: Option<String>,
        is_revealed: bool,
    },
    Text {
        default: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormField {
    pub name: String,
    pub label: String,
    pub help_summary: String,
    pub help_details: Option<String>,
    pub required: bool,
    pub advanced: bool,
    pub group: String,
    pub provider: Option<String>,
    pub value: String,
    pub kind: FormFieldKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct FormSchema {
    pub fields: Vec<FormField>,
}

impl FormSchema {
    pub fn new() -> Self {
        Self { fields: Vec::new() }
    }

    pub fn field(&self, name: &str) -> Option<&FormField> {
        self.fields.iter().find(|f| f.name == name)
    }

    pub fn field_mut(&mut self, name: &str) -> Option<&mut FormField> {
        self.fields.iter_mut().find(|f| f.name == name)
    }

    pub fn set_value(&mut self, name: &str, val: impl Into<String>) {
        if let Some(f) = self.field_mut(name) {
            f.value = val.into();
        }
    }

    /// Schema-driven form generation from OptionSchema metadata (R3, §7.3)
    pub fn from_options(options: &[OptionSchema]) -> Self {
        let mut fields = Vec::with_capacity(options.len());

        for opt in options {
            if opt.hide > 0 {
                // Hidden options only shown in "show all" mode
                continue;
            }

            // Split Help into first line (inline hint) and remainder (popover details)
            let help_trimmed = opt.help.trim();
            let (help_summary, help_details) = if let Some(newline_pos) = help_trimmed.find('\n') {
                let summary = help_trimmed[..newline_pos].trim().to_string();
                let details = help_trimmed[newline_pos + 1..].trim().to_string();
                let details_opt = if details.is_empty() { None } else { Some(details) };
                (summary, details_opt)
            } else {
                (help_trimmed.to_string(), None)
            };

            let default_str = opt.default.as_ref().and_then(|d| match d {
                serde_json::Value::String(s) => Some(s.clone()),
                serde_json::Value::Bool(b) => Some(b.to_string()),
                serde_json::Value::Number(n) => Some(n.to_string()),
                _ => None,
            });

            let kind = if opt.is_password || opt.sensitive {
                FormFieldKind::Secret {
                    default: default_str.clone(),
                    is_revealed: false,
                }
            } else if opt.option_type == "bool" {
                let default_bool = opt.default.as_ref().and_then(|d| d.as_bool()).unwrap_or(false);
                FormFieldKind::Toggle { default: default_bool }
            } else if opt.exclusive && !opt.examples.is_empty() {
                let options_list: Vec<(String, String)> = opt
                    .examples
                    .iter()
                    .map(|e| (e.value.clone(), e.help.clone()))
                    .collect();
                FormFieldKind::Select {
                    options: options_list,
                    default: default_str.clone(),
                }
            } else if !opt.exclusive && !opt.examples.is_empty() {
                let suggestions: Vec<String> = opt.examples.iter().map(|e| e.value.clone()).collect();
                FormFieldKind::ComboBox {
                    suggestions,
                    default: default_str.clone(),
                }
            } else if is_numeric_or_unit_type(&opt.option_type) {
                FormFieldKind::NumberWithUnit {
                    unit_type: opt.option_type.clone(),
                    default: default_str.clone(),
                }
            } else {
                FormFieldKind::Text {
                    default: default_str.clone(),
                }
            };

            let initial_value = default_str.unwrap_or_default();

            let field = FormField {
                name: opt.name.clone(),
                label: format_label(&opt.name),
                help_summary,
                help_details,
                required: opt.required,
                advanced: opt.advanced,
                group: opt.groups.clone(),
                provider: if opt.provider.is_empty() { None } else { Some(opt.provider.clone()) },
                value: initial_value,
                kind,
            };

            fields.push(field);
        }

        Self { fields }
    }

    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        for f in &self.fields {
            if f.required && f.value.trim().is_empty() {
                errors.push(format!("'{}' is a required field", f.label));
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    pub fn to_json_map(&self) -> BTreeMap<String, serde_json::Value> {
        let mut map = BTreeMap::new();
        for f in &self.fields {
            if !f.value.is_empty() {
                // If boolean toggle
                if let FormFieldKind::Toggle { .. } = f.kind {
                    let b = f.value.parse::<bool>().unwrap_or(false);
                    map.insert(f.name.clone(), serde_json::Value::Bool(b));
                } else {
                    map.insert(f.name.clone(), serde_json::Value::String(f.value.clone()));
                }
            }
        }
        map
    }
}

fn is_numeric_or_unit_type(t: &str) -> bool {
    matches!(
        t,
        "int" | "float" | "SizeSuffix" | "Duration" | "BandwidthSpec" | "Tristate"
    )
}

fn format_label(name: &str) -> String {
    name.replace('_', " ")
        .split_whitespace()
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
