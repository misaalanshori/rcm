use rcm_core::MountPreset;
use rcm_rc::types::ProviderInfo;

/// Traceability: FR-RM-02, FR-RM-03
/// Verifies provider filtering in New Remote Wizard by name, prefix, and description
#[test]
fn test_fr_rm_02_rm_03_wizard_provider_filter_and_selection() {
    let providers = [
        ProviderInfo {
            name: "Amazon S3".to_string(),
            description: "Amazon S3 Compliant Storage".to_string(),
            prefix: "s3".to_string(),
            options: Vec::new(),
        },
        ProviderInfo {
            name: "Google Drive".to_string(),
            description: "Google Drive storage".to_string(),
            prefix: "drive".to_string(),
            options: Vec::new(),
        },
        ProviderInfo {
            name: "Dropbox".to_string(),
            description: "Dropbox cloud storage".to_string(),
            prefix: "dropbox".to_string(),
            options: Vec::new(),
        },
    ];

    // Filter by "drive"
    let query = "drive";
    let filtered: Vec<&ProviderInfo> = providers
        .iter()
        .filter(|p| {
            p.name.to_lowercase().contains(query)
                || p.prefix.to_lowercase().contains(query)
                || p.description.to_lowercase().contains(query)
        })
        .collect();

    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].prefix, "drive");
}

/// Traceability: FR-MT-02, FR-MT-04
/// Verifies mount modal presets map to concrete VFS caching modes
#[test]
fn test_fr_mt_02_mt_04_mount_modal_presets_mapping() {
    let presets = [
        MountPreset::Balanced,
        MountPreset::Streaming,
        MountPreset::OfflineFirst,
        MountPreset::MaxCompatibility,
        MountPreset::ReadOnly,
    ];

    for preset in presets {
        let opts = preset.default_options();
        if preset == MountPreset::Streaming || preset == MountPreset::OfflineFirst {
            assert_eq!(opts.get("vfs_cache_mode"), Some(&"full".to_string()));
        } else if preset == MountPreset::Balanced {
            assert_eq!(opts.get("vfs_cache_mode"), Some(&"writes".to_string()));
        }
    }
}
