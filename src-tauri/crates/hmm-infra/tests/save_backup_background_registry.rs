use hmm_core::SaveBackupBackgroundRegistrationStatus;
use hmm_infra::UnsupportedSaveBackupBackgroundRegistry;
use hmm_ports::SaveBackupBackgroundRegistry;

#[test]
fn unsupported_registry_returns_unsupported_platform_for_all_operations() {
    let registry = UnsupportedSaveBackupBackgroundRegistry;

    assert_eq!(
        registry.inspect().expect("inspect succeeds"),
        SaveBackupBackgroundRegistrationStatus::UnsupportedPlatform
    );
    assert_eq!(
        registry.register().expect("register succeeds"),
        SaveBackupBackgroundRegistrationStatus::UnsupportedPlatform
    );
    assert_eq!(
        registry.unregister().expect("unregister succeeds"),
        SaveBackupBackgroundRegistrationStatus::UnsupportedPlatform
    );
}
