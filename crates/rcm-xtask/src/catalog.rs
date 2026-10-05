use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandDoc {
    pub name: String,
    pub description: String,
    pub args: Vec<String>,
    pub flags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CommandCatalog {
    pub commands: BTreeMap<String, CommandDoc>,
}

impl CommandCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_command(&mut self, doc: CommandDoc) {
        self.commands.insert(doc.name.clone(), doc);
    }

    pub fn get(&self, name: &str) -> Option<&CommandDoc> {
        self.commands.get(name)
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }

    /// Pre-populated catalog covering Appendix A commands
    pub fn default_catalog() -> Self {
        let mut c = Self::new();

        c.add_command(CommandDoc {
            name: "about".to_string(),
            description: "Get quota information from the remote.".to_string(),
            args: vec!["remote:path".to_string()],
            flags: vec!["json".to_string(), "full".to_string()],
        });

        c.add_command(CommandDoc {
            name: "check".to_string(),
            description: "Checks the files in the source and destination match.".to_string(),
            args: vec!["source:path".to_string(), "dest:path".to_string()],
            flags: vec!["size-only".to_string(), "download".to_string(), "one-way".to_string(), "differ".to_string()],
        });

        c.add_command(CommandDoc {
            name: "cryptcheck".to_string(),
            description: "Cryptcheck checks the integrity of a crypted remote.".to_string(),
            args: vec!["source:path".to_string(), "dest:path".to_string()],
            flags: vec!["size-only".to_string(), "differ".to_string()],
        });

        c.add_command(CommandDoc {
            name: "dedupe".to_string(),
            description: "Interactively finds duplicate files and offers to delete or rename them.".to_string(),
            args: vec!["remote:path".to_string()],
            flags: vec!["dedupe-mode".to_string()],
        });

        c.add_command(CommandDoc {
            name: "hashsum".to_string(),
            description: "Produces a hashsum file for all the objects in the path.".to_string(),
            args: vec!["hash".to_string(), "remote:path".to_string()],
            flags: vec!["output-file".to_string()],
        });

        c.add_command(CommandDoc {
            name: "cleanup".to_string(),
            description: "Clean up the remote if possible (empty trash, delete untracked data).".to_string(),
            args: vec!["remote:path".to_string()],
            flags: Vec::new(),
        });

        c.add_command(CommandDoc {
            name: "settier".to_string(),
            description: "Changes storage tier of objects in remote.".to_string(),
            args: vec!["tier".to_string(), "remote:path".to_string()],
            flags: Vec::new(),
        });

        c
    }
}
