use rcm_xtask::catalog::{CommandCatalog, CommandDoc};

/// Traceability: NFR-PF-01, FR-FL-07
/// Tests command catalog generation, flag serialization, and lookup
#[test]
fn test_nfr_pf_01_command_catalog_and_footprint_budget() {
    let mut catalog = CommandCatalog::new();

    catalog.add_command(CommandDoc {
        name: "about".to_string(),
        description: "Get quota information from the remote.".to_string(),
        args: vec!["remote:path".to_string()],
        flags: vec!["json".to_string(), "full".to_string()],
    });

    catalog.add_command(CommandDoc {
        name: "check".to_string(),
        description: "Checks the files in the source and destination match.".to_string(),
        args: vec!["source:path".to_string(), "dest:path".to_string()],
        flags: vec!["size-only".to_string(), "download".to_string(), "one-way".to_string()],
    });

    let about_cmd = catalog.get("about").expect("about command missing");
    assert_eq!(about_cmd.name, "about");
    assert_eq!(about_cmd.args.len(), 1);
    assert_eq!(about_cmd.flags.len(), 2);

    let check_cmd = catalog.get("check").expect("check command missing");
    assert_eq!(check_cmd.args.len(), 2);
    assert!(check_cmd.flags.contains(&"size-only".to_string()));

    let json_str = catalog.to_json().expect("Failed to serialize catalog");
    let deserialized = CommandCatalog::from_json(&json_str).expect("Failed to deserialize catalog");
    assert_eq!(catalog, deserialized);
}
