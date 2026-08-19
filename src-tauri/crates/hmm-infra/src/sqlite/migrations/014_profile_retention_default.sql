-- 把 retention_max_count 的列默认值从 20 改成 0（0 表示不限制），与
-- hmm_core::ProfileBackupRetention::default() 对齐。
--
-- 背景：默认保留策略早期是 20 份 / 30 天，后来改为不限制，但只改了 Rust 端，
-- 建表 SQL 的 DEFAULT 20 留了下来。生产的 upsert 每次都显式写这一列，所以旧默认
-- 目前打不中；但只要将来新增一条省略该列的插入，新行就会静默变成 20 份上限，
-- 而玩家从 UI 上看不出自己被限制了。这是玩家数据相关的静默行为差异，必须消除。
--
-- SQLite 无法直接修改列默认值，只能重建表。本表没有入向外键、没有索引和触发器，
-- 唯一的外键是指向 profiles 的出向引用，因此重建是安全的。
--
-- 刻意保留既有行的值：玩家可能确实手动设过 20 份，静默改写会让他们莫名其妙
-- 失去已生效的保留策略。本迁移只改"以后新行取什么默认值"。

CREATE TABLE profile_save_settings_rebuilt (
    profile_id                 TEXT    PRIMARY KEY NOT NULL,
    save_directory             TEXT,
    backup_directory           TEXT,
    backup_cadence             TEXT    NOT NULL DEFAULT 'manual',
    backup_hour                INTEGER,
    backup_minute              INTEGER,
    backup_weekdays            TEXT    NOT NULL DEFAULT '[]',
    retention_max_count        INTEGER NOT NULL DEFAULT 0,
    retention_max_age_days     INTEGER,
    updated_at                 INTEGER NOT NULL,
    pre_restore_backup_enabled INTEGER NOT NULL DEFAULT 1,
    retention_max_total_bytes  INTEGER
        CHECK (retention_max_total_bytes IS NULL
            OR retention_max_total_bytes BETWEEN 16777216 AND 1099511627776),
    steam_account_name         TEXT,
    steam_avatar_url           TEXT,
    steam_account_label        TEXT,
    FOREIGN KEY(profile_id) REFERENCES profiles(profile_id) ON DELETE CASCADE
);

INSERT INTO profile_save_settings_rebuilt (
    profile_id, save_directory, backup_directory, backup_cadence,
    backup_hour, backup_minute, backup_weekdays,
    retention_max_count, retention_max_age_days, updated_at,
    pre_restore_backup_enabled, retention_max_total_bytes,
    steam_account_name, steam_avatar_url, steam_account_label
)
SELECT
    profile_id, save_directory, backup_directory, backup_cadence,
    backup_hour, backup_minute, backup_weekdays,
    retention_max_count, retention_max_age_days, updated_at,
    pre_restore_backup_enabled, retention_max_total_bytes,
    steam_account_name, steam_avatar_url, steam_account_label
FROM profile_save_settings;

DROP TABLE profile_save_settings;

ALTER TABLE profile_save_settings_rebuilt RENAME TO profile_save_settings;
