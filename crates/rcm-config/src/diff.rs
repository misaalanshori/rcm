use std::collections::BTreeSet;
use crate::ini::IniConfig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffOp {
    Added(String),
    Removed(String),
    Modified { old: String, new: String },
    Unchanged(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionDiff {
    pub name: String,
    pub is_new_section: bool,
    pub is_deleted_section: bool,
    pub property_diffs: Vec<(String, DiffOp)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RedactedDiff {
    pub section_diffs: Vec<SectionDiff>,
}

impl RedactedDiff {
    pub fn render_redacted(&self) -> String {
        let mut out = String::new();
        for sec in &self.section_diffs {
            if sec.is_new_section {
                out.push_str(&format!("+[ {} ]\n", sec.name));
            } else if sec.is_deleted_section {
                out.push_str(&format!("-[ {} ]\n", sec.name));
            } else {
                out.push_str(&format!(" [ {} ]\n", sec.name));
            }

            for (k, op) in &sec.property_diffs {
                let is_sensitive = is_sensitive_key(k);
                match op {
                    DiffOp::Added(val) => {
                        let display_val = if is_sensitive { "***REDACTED***" } else { val };
                        out.push_str(&format!("+ {} = {}\n", k, display_val));
                    }
                    DiffOp::Removed(val) => {
                        let display_val = if is_sensitive { "***REDACTED***" } else { val };
                        out.push_str(&format!("- {} = {}\n", k, display_val));
                    }
                    DiffOp::Modified { old, new } => {
                        let display_old = if is_sensitive { "***REDACTED***" } else { old };
                        let display_new = if is_sensitive { "***REDACTED***" } else { new };
                        out.push_str(&format!("- {} = {}\n", k, display_old));
                        out.push_str(&format!("+ {} = {}\n", k, display_new));
                    }
                    DiffOp::Unchanged(val) => {
                        let display_val = if is_sensitive { "***REDACTED***" } else { val };
                        out.push_str(&format!("  {} = {}\n", k, display_val));
                    }
                }
            }
            out.push('\n');
        }
        out
    }
}

pub fn is_sensitive_key(key: &str) -> bool {
    let k = key.to_lowercase();
    k.contains("pass")
        || k.contains("secret")
        || k.contains("token")
        || k.contains("key")
        || k.contains("auth")
        || k.contains("credential")
}

pub fn diff_configs(old_conf: &IniConfig, new_conf: &IniConfig) -> RedactedDiff {
    let mut all_section_names = BTreeSet::new();
    for name in old_conf.sections.keys() {
        all_section_names.insert(name.clone());
    }
    for name in new_conf.sections.keys() {
        all_section_names.insert(name.clone());
    }

    let mut section_diffs = Vec::new();

    for name in all_section_names {
        match (old_conf.section(&name), new_conf.section(&name)) {
            (Some(old_sec), Some(new_sec)) => {
                let mut prop_keys = BTreeSet::new();
                for k in old_sec.properties.keys() {
                    prop_keys.insert(k.clone());
                }
                for k in new_sec.properties.keys() {
                    prop_keys.insert(k.clone());
                }

                let mut property_diffs = Vec::new();
                let mut has_changes = false;

                for k in prop_keys {
                    match (old_sec.get(&k), new_sec.get(&k)) {
                        (Some(o), Some(n)) => {
                            if o != n {
                                has_changes = true;
                                property_diffs.push((
                                    k,
                                    DiffOp::Modified {
                                        old: o.to_string(),
                                        new: n.to_string(),
                                    },
                                ));
                            } else {
                                property_diffs.push((k, DiffOp::Unchanged(o.to_string())));
                            }
                        }
                        (Some(o), None) => {
                            has_changes = true;
                            property_diffs.push((k, DiffOp::Removed(o.to_string())));
                        }
                        (None, Some(n)) => {
                            has_changes = true;
                            property_diffs.push((k, DiffOp::Added(n.to_string())));
                        }
                        (None, None) => {}
                    }
                }

                if has_changes {
                    section_diffs.push(SectionDiff {
                        name,
                        is_new_section: false,
                        is_deleted_section: false,
                        property_diffs,
                    });
                }
            }
            (None, Some(new_sec)) => {
                let mut property_diffs = Vec::new();
                for (k, v) in &new_sec.properties {
                    property_diffs.push((k.clone(), DiffOp::Added(v.clone())));
                }
                section_diffs.push(SectionDiff {
                    name,
                    is_new_section: true,
                    is_deleted_section: false,
                    property_diffs,
                });
            }
            (Some(old_sec), None) => {
                let mut property_diffs = Vec::new();
                for (k, v) in &old_sec.properties {
                    property_diffs.push((k.clone(), DiffOp::Removed(v.clone())));
                }
                section_diffs.push(SectionDiff {
                    name,
                    is_new_section: false,
                    is_deleted_section: true,
                    property_diffs,
                });
            }
            (None, None) => {}
        }
    }

    RedactedDiff { section_diffs }
}
