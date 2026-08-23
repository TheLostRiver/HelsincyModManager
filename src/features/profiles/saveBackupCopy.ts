import type { LocaleDictionary } from "../../shared/i18n";
import type { ProfileSaveBackupTaskPhase } from "./profileSaveBackupTaskState";

// 存档备份任务（阶段标签 + 稳定错误码文案）的全部用户可见文案。
// 语义推进留在 profileSaveBackupTaskState：state 只存 phase/errorCode，
// 文本在渲染或 toast 组装时经本字典取。

export type SaveBackupErrorCopy = {
  byCode: Record<string, string>;
  fallback: string;
};

export type SaveBackupCopy = {
  phases: Record<ProfileSaveBackupTaskPhase, string>;
  errors: SaveBackupErrorCopy;
};

export const saveBackupCopy = {
  zh_cn: {
    phases: {
      "save_backup.queued": "等待备份",
      "save_backup.scanning": "校验存档",
      "save_backup.archiving": "写入归档",
      "save_backup.manifest_writing": "写入备份清单",
      "save_backup.retention_pruning": "清理旧备份",
      "save_backup.completed": "备份完成",
      "save_backup.failed": "备份失败",
      "save_backup.cancelled": "已取消",
    },
    errors: {
      byCode: {
        write_admission_busy: "另一项存档操作正在进行，请稍后再试。",
        write_admission_cancelled: "存档备份已取消。",
        write_admission_order_violation: "存档操作顺序发生变化，请稍后重试。",
        write_admission_unavailable: "暂时无法锁定存档写入，请稍后重试。",
        save_backup_profile_missing: "当前配置档已不存在，请刷新后重试。",
        save_backup_source_unset: "当前配置档尚未设置存档目录。",
        save_backup_source_invalid: "当前配置档的存档目录无效，请先重新设置。",
        save_backup_clock_unavailable: "无法建立可靠的备份时间，请稍后重试。",
        save_backup_destination_unavailable: "备份目录当前不可用，请检查目录设置。",
        save_backup_archive_write_failed: "无法写入存档备份，请检查备份目录。",
        save_backup_history_unavailable: "备份历史当前不可用，请稍后重试。",
        save_backup_retention_failed: "备份保留策略执行失败，请检查备份中心。",
        save_backup_scheduler_lease_unavailable: "自动备份调度状态暂时不可用，请稍后重试。",
      },
      fallback: "存档备份失败，请稍后重试。",
    },
  },
  en: {
    phases: {
      "save_backup.queued": "Waiting to back up",
      "save_backup.scanning": "Validating save data",
      "save_backup.archiving": "Writing archive",
      "save_backup.manifest_writing": "Writing backup manifest",
      "save_backup.retention_pruning": "Pruning old backups",
      "save_backup.completed": "Backup completed",
      "save_backup.failed": "Backup failed",
      "save_backup.cancelled": "Cancelled",
    },
    errors: {
      byCode: {
        write_admission_busy: "Another save data operation is in progress. Please try again later.",
        write_admission_cancelled: "Save data backup was cancelled.",
        write_admission_order_violation: "The order of save data operations changed. Please try again later.",
        write_admission_unavailable: "Could not lock save data writes for now. Please try again later.",
        save_backup_profile_missing: "The current profile no longer exists. Refresh and try again.",
        save_backup_source_unset: "The current profile has no save data directory yet.",
        save_backup_source_invalid: "The save data directory of the current profile is invalid. Set it again first.",
        save_backup_clock_unavailable: "Could not establish a reliable backup time. Please try again later.",
        save_backup_destination_unavailable: "The backup directory is currently unavailable. Check the directory settings.",
        save_backup_archive_write_failed: "Could not write the save data backup. Check the backup directory.",
        save_backup_history_unavailable: "Backup history is currently unavailable. Please try again later.",
        save_backup_retention_failed: "Applying the backup retention policy failed. Check the backup center.",
        save_backup_scheduler_lease_unavailable: "Auto backup scheduler state is temporarily unavailable. Please try again later.",
      },
      fallback: "Save data backup failed. Please try again later.",
    },
  },
  ja: {
    phases: {
      "save_backup.queued": "バックアップ待機中",
      "save_backup.scanning": "セーブデータを検証中",
      "save_backup.archiving": "アーカイブを書き込み中",
      "save_backup.manifest_writing": "バックアップマニフェストを書き込み中",
      "save_backup.retention_pruning": "古いバックアップを整理中",
      "save_backup.completed": "バックアップ完了",
      "save_backup.failed": "バックアップ失敗",
      "save_backup.cancelled": "キャンセル済み",
    },
    errors: {
      byCode: {
        write_admission_busy: "別のセーブデータ操作が進行中です。しばらくしてから再試行してください。",
        write_admission_cancelled: "セーブデータのバックアップをキャンセルしました。",
        write_admission_order_violation: "セーブデータ操作の順序が変化しました。しばらくしてから再試行してください。",
        write_admission_unavailable: "現在セーブデータ書き込みをロックできません。しばらくしてから再試行してください。",
        save_backup_profile_missing: "現在のプロファイルは存在しません。更新してから再試行してください。",
        save_backup_source_unset: "現在のプロファイルにはセーブデータディレクトリが未設定です。",
        save_backup_source_invalid: "現在のプロファイルのセーブデータディレクトリが無効です。先に設定し直してください。",
        save_backup_clock_unavailable: "信頼できるバックアップ時刻を確立できません。しばらくしてから再試行してください。",
        save_backup_destination_unavailable: "バックアップディレクトリが現在利用できません。ディレクトリ設定を確認してください。",
        save_backup_archive_write_failed: "セーブデータのバックアップを書き込めません。バックアップディレクトリを確認してください。",
        save_backup_history_unavailable: "バックアップ履歴が現在利用できません。しばらくしてから再試行してください。",
        save_backup_retention_failed: "バックアップ保持ポリシーの実行に失敗しました。バックアップセンターを確認してください。",
        save_backup_scheduler_lease_unavailable: "自動バックアップのスケジューラ状態が一時的に利用できません。しばらくしてから再試行してください。",
      },
      fallback: "セーブデータのバックアップに失敗しました。しばらくしてから再試行してください。",
    },
  },
} satisfies LocaleDictionary<SaveBackupCopy>;
