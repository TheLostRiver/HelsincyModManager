# Install Safety Checklist

Use this checklist for HMM install, uninstall, backup, rollback, staging, archive, and save-safety work.

## Data Boundary

- Original imported Mod package remains read-only.
- Extraction target is sandbox/cache, never a game or save directory.
- Retarget/materialized variants write only to staging.
- Staging can be deleted and rebuilt; it is not the installation fact source.
- Real game writes happen only after `InstallPlan` and conflict/dependency checks.
- Default save backups live outside the game install directory.
- Save restore reads from a manifest-backed backup and validates before writing.

## Path Safety

- Normalize separators before comparing logical paths.
- Reject `..`, absolute paths, drive prefixes, UNC paths, empty segments that change meaning, symlink/junction escape, and case-insensitive collisions.
- Verify final resolved filesystem target remains under the intended root before write/delete.
- Conflict detection uses final target paths after retarget/staging, not original archive paths.

## Install/Uninstall Chain

- Install: analyze -> build `InstallPlan` -> conflict/dependency checks -> backup -> commit -> manifest -> rollback/recover path.
- Overwrite: existing file is backed up before replacement.
- Manifest: records enough information to uninstall and recover without re-reading a third-party archive.
- Uninstall: remove only manifest-owned files; preserve unknown user/game files.
- Failure: rollback best effort; record whether rollback succeeded or failed.

## Save Backup/Restore

- Support the default backup directory and a player-selected backup directory.
- Keep backup results manifest-backed with hashes or equivalent validation data.
- Require restore validation and explicit confirmation before writing to a save location.
- Preserve automatic backup interval and retention settings when touched.
- Cover unwritable backup directories and retention limits.
- Use temp save directories only; never read or write real player saves in automated tests.

## Logging

- Before real game directory writes, require logging/telemetry initialization, `task_id` generation and propagation, redaction helpers, log directory resolution, Audit Log writer, and tests for redaction/audit events.
- Audit Log required for game directory writes, overwrites, deletes, backup, restore, manifest, rollback, and recovery.
- Log task id, game id, profile/mod id, logical target, hash/size, result, and error classification when available.
- Redact full local paths, usernames, Steam IDs, tokens, cookies, real save content, and third-party Mod content.

## Test Fixtures

- Use temp directories, fake file systems, and artificial tiny package fixtures.
- Do not require real MHW:I, real save directories, or real third-party Mod packages for automated tests.
- Assert temp game directory state after success, failure, uninstall, and rollback.
- Add regression tests for any reported data-loss, escape, collision, or partial-manifest bug.
