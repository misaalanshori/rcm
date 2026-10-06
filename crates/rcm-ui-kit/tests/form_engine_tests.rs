use rcm_rc::types::{OptionExample, OptionSchema};
use rcm_ui_kit::form::{FormFieldKind, FormSchema};

/// Traceability: FR-RM-06
/// Verifies schema-driven form field generation from rclone OptionSchema metadata
#[test]
fn test_fr_rm_06_form_engine_generation_from_option_schema() {
    let options = vec![
        OptionSchema {
            name: "provider".to_string(),
            help: "Choose your S3 provider.\nEnter AWS, MinIO, etc.".to_string(),
            default: Some(serde_json::Value::String("AWS".to_string())),
            examples: vec![
                OptionExample {
                    value: "AWS".to_string(),
                    help: "Amazon Web Services (AWS) S3".to_string(),
                    provider: "".to_string(),
                },
                OptionExample {
                    value: "Minio".to_string(),
                    help: "MinIO Object Storage".to_string(),
                    provider: "".to_string(),
                },
            ],
            required: true,
            is_password: false,
            sensitive: false,
            advanced: false,
            exclusive: true,
            option_type: "string".to_string(),
            groups: "".to_string(),
            provider: "".to_string(),
            hide: 0,
        },
        OptionSchema {
            name: "secret_access_key".to_string(),
            help: "AWS Secret Access Key.\nLeave blank for IAM role.".to_string(),
            default: None,
            examples: Vec::new(),
            required: false,
            is_password: true,
            sensitive: true,
            advanced: false,
            exclusive: false,
            option_type: "string".to_string(),
            groups: "".to_string(),
            provider: "".to_string(),
            hide: 0,
        },
        OptionSchema {
            name: "vfs_cache_mode".to_string(),
            help: "Cache mode (off, minimal, writes, full)".to_string(),
            default: Some(serde_json::Value::String("writes".to_string())),
            examples: vec![
                OptionExample { value: "off".to_string(), help: "No cache".to_string(), provider: "".to_string() },
                OptionExample { value: "writes".to_string(), help: "Cache writes".to_string(), provider: "".to_string() },
                OptionExample { value: "full".to_string(), help: "Full cache".to_string(), provider: "".to_string() },
            ],
            required: false,
            is_password: false,
            sensitive: false,
            advanced: true,
            exclusive: true,
            option_type: "string".to_string(),
            groups: "VFS".to_string(),
            provider: "".to_string(),
            hide: 0,
        },
    ];

    let schema = FormSchema::from_options(&options);
    assert_eq!(schema.fields.len(), 3);

    // 1. Exclusive + Examples -> Select
    let provider_field = schema.field("provider").expect("provider field missing");
    assert!(provider_field.required);
    assert!(!provider_field.advanced);
    assert_eq!(provider_field.help_summary, "Choose your S3 provider.");
    assert_eq!(provider_field.help_details.as_deref(), Some("Enter AWS, MinIO, etc."));
    match &provider_field.kind {
        FormFieldKind::Select { options, default } => {
            assert_eq!(options.len(), 2);
            assert_eq!(options[0].0, "AWS");
            assert_eq!(default.as_deref(), Some("AWS"));
        }
        _ => panic!("Expected Select kind for provider"),
    }

    // 2. is_password: true -> Secret kind (masked)
    let secret_field = schema.field("secret_access_key").expect("secret missing");
    match &secret_field.kind {
        FormFieldKind::Secret { is_revealed, .. } => {
            assert!(!is_revealed, "Secret field must not be revealed by default (NFR-SC-04)");
        }
        _ => panic!("Expected Secret kind"),
    }

    // 3. advanced: true -> Collapsible group
    let vfs_field = schema.field("vfs_cache_mode").expect("vfs missing");
    assert!(vfs_field.advanced);
    assert_eq!(vfs_field.group, "VFS");
}

/// Traceability: FR-RM-04
/// Verifies form schema required field validation and JSON map export
#[test]
fn test_fr_rm_04_form_schema_validation_and_export() {
    let options = vec![
        OptionSchema {
            name: "bucket".to_string(),
            help: "Target bucket name".to_string(),
            default: None,
            examples: Vec::new(),
            required: true,
            is_password: false,
            sensitive: false,
            advanced: false,
            exclusive: false,
            option_type: "string".to_string(),
            groups: "".to_string(),
            provider: "".to_string(),
            hide: 0,
        },
    ];

    let mut schema = FormSchema::from_options(&options);

    // Initially empty value -> required validation fails
    let val_err = schema.validate();
    assert!(val_err.is_err());

    // Fill valid value -> validation succeeds
    schema.set_value("bucket", "my-data-bucket");
    assert!(schema.validate().is_ok());

    let map = schema.to_json_map();
    assert_eq!(map.get("bucket"), Some(&serde_json::Value::String("my-data-bucket".to_string())));
}
