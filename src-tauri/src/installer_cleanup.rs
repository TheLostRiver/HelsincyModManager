use hmm_infra::{cleanup_owned_save_backup_task_for_installer, InstallerCleanupOutcome};
use std::ffi::OsString;

pub fn installer_cleanup_exit_code(outcome: InstallerCleanupOutcome) -> i32 {
    match outcome {
        InstallerCleanupOutcome::Removed
        | InstallerCleanupOutcome::AlreadyAbsent
        | InstallerCleanupOutcome::ForeignPreserved => 0,
        InstallerCleanupOutcome::OwnedTaskRunning => 20,
        InstallerCleanupOutcome::OwnershipUnverified => 21,
        InstallerCleanupOutcome::RemovalUnverified => 22,
        InstallerCleanupOutcome::PlatformUnavailable => 23,
    }
}

pub fn installer_cleanup_entry<I, T, F>(args: I, cleanup: F) -> i32
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
    F: FnOnce() -> InstallerCleanupOutcome,
{
    let mut args = args.into_iter().map(Into::into);
    let Some(_program) = args.next() else {
        return 64;
    };
    if args.next().is_some() {
        return 64;
    }
    installer_cleanup_exit_code(cleanup())
}

pub fn run_installer_cleanup_from_env() -> i32 {
    installer_cleanup_entry(
        std::env::args_os(),
        cleanup_owned_save_backup_task_for_installer,
    )
}

#[cfg(test)]
mod tests {
    use super::{installer_cleanup_entry, installer_cleanup_exit_code};
    use hmm_infra::InstallerCleanupOutcome;
    use std::cell::Cell;

    #[test]
    fn installer_cleanup_maps_all_typed_outcomes_to_stable_exit_codes() {
        for (outcome, expected) in [
            (InstallerCleanupOutcome::Removed, 0),
            (InstallerCleanupOutcome::AlreadyAbsent, 0),
            (InstallerCleanupOutcome::ForeignPreserved, 0),
            (InstallerCleanupOutcome::OwnedTaskRunning, 20),
            (InstallerCleanupOutcome::OwnershipUnverified, 21),
            (InstallerCleanupOutcome::RemovalUnverified, 22),
            (InstallerCleanupOutcome::PlatformUnavailable, 23),
        ] {
            assert_eq!(installer_cleanup_exit_code(outcome), expected);
        }
    }

    #[test]
    fn installer_cleanup_accepts_no_arguments_and_rejects_any_extra_argument() {
        assert_eq!(
            installer_cleanup_entry(["hmm-save-backup-installer-cleanup"], || {
                InstallerCleanupOutcome::AlreadyAbsent
            }),
            0
        );

        let called = Cell::new(false);
        let exit_code =
            installer_cleanup_entry(["hmm-save-backup-installer-cleanup", "--task-name"], || {
                called.set(true);
                InstallerCleanupOutcome::AlreadyAbsent
            });
        assert_eq!(exit_code, 64);
        assert!(!called.get(), "invalid invocation must not call cleanup");
    }
}
