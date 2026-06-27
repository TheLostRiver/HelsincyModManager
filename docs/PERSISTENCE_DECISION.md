# 持久化方案决策

创建时间：2026-06-27
关联任务：TODO.md T2

---

## 1. 现状

### 已落地的仓储

| 仓储 trait | 实现 | 存储方式 | 文件位置 |
|---|---|---|---|
| `GameConfigRepository` | `JsonGameConfigRepository` | 单 JSON 文件 | `config/games.json` |
| `AppSettingsRepository` | `JsonAppSettingsRepository` | 单 JSON 文件 | `config/settings.json` |
| `ModImportResultRepository` | `JsonModImportResultRepository` | 单 JSON 文件 | `mod-import/results.json` |
| `InstallManifestRepository` | `JsonInstallManifestRepository` | 每 profile 一个 JSON | `install/manifests/{profile_id}.json` |
| `InstallRecoveryRecordRepository` | `JsonInstallRecoveryRecordRepository` | 每 (profile, mod) 一个 JSON | `install/recovery/record-{hash}.json` |
| `AuditLogWriter` / `AuditLogReader` | `FileSystemAuditLogWriter` | JSONL 日志文件 | `logs/audit/audit-YYYY-MM-DD.log` |

### 写入安全模式

所有 JSON 仓储共享一套经过验证的写入安全模式：

- **原子写入**：先写临时文件 → `sync_all()` → `rename()` → 目录 fsync
- **进程内串行**：`Mutex<()>` 防止并发写入
- **跨进程锁**（部分仓储）：`fs2::FileExt::lock_exclusive()`
- **Schema 版本**：`version: u32` 字段，不匹配视为损坏
- **符号链接防御**：`symlink_metadata()` + 规范化路径

### JSON 做得好的场景

| 场景 | 原因 |
|---|---|
| 游戏配置（1 个实例） | 数据量极小，整文件读写无压力 |
| 应用设置（2 个字段） | 极简 KV |
| 安装 manifest（按 profile 分文件） | 追加式条目，整文件读写可接受 |
| 恢复记录（按 mod 分文件） | 独立文件，天然隔离 |
| 审计日志（JSONL 按天分文件） | 只追加，不查询 |

### JSON 做不好的场景

| 需求 | 困难点 | 对应任务 |
|---|---|---|
| Mod 元数据编辑（overlay） | 导入快照是只读的，用户编辑需要额外层或就地修改 | T3 |
| 分类标签多对多 | `Mod ↔ Category` 关系查询和级联删除困难 | T4 |
| Profile CRUD + 关联 | Profile 需要成为实体，关联 manifest/binding/分类 | T6 |
| ReplacementBinding | `Mod × Profile × ReplacementTarget` 三方关系 + 冲突检测查询 | T11 |
| 备份历史 | 时间线查询、保留策略执行 | T8 |
| Mod 导入结果 | 单文件 `results.json` 随 Mod 数量无限增长，每次写入重序列化全部 | — |

---

## 2. 决策

**SQLite 管理用户可编辑的关系数据，JSON 保留安装链路数据。两套共存，不迁移。**

### SQLite 覆盖范围

- Mod 元数据 overlay（用户编辑的名称、作者、版本、备注、NexusMods ID）
- 分类和标签（多对多关系）
- Profile（实体化 + CRUD）
- ReplacementBinding（三方关系）
- 备份历史（时间线查询）

### JSON 保留范围

- `GameConfigRepository` — 数据量极小，无关系需求
- `AppSettingsRepository` — 极简 KV
- `InstallManifestRepository` — 安装链路崩溃恢复关键，temp-write-rename 已验证稳定
- `InstallRecoveryRecordRepository` — 同上
- `AuditLogWriter` / `AuditLogReader` — JSONL 只追加，不需要查询索引
- `ModImportResultRepository` — 暂保留，但 `results.json` 的增长问题可在后续考虑迁移

### 理由

1. 六边形架构已将持久化隔离在 `hmm-ports` trait 之后，新增 SQLite 实现只是 `hmm-infra` 的内部事务
2. 安装链路的 JSON 仓储经过 PR #85–#108 的 22 个 PR 验证，迁移无用户收益但有引入回归的风险
3. 分类、Profile、ReplacementBinding 是天然的关系模型，用 JSON 实现会产生大量手工 join 和一致性维护代码
4. `ARCHITECTURE.md` 数据存储章节已将 SQLite 列为架构规划

---

## 3. 库选型

### 选定

| 库 | 版本 | 用途 |
|---|---|---|
| `rusqlite` | `0.35` + `features = ["bundled"]` | SQLite 操作，内嵌 SQLite 库 |
| `rusqlite_migration` | `2.5` | 基于 `PRAGMA user_version` 的前向迁移 |

### 排除

| 库 | 排除原因 |
|---|---|
| `sqlx` | 异步运行时开销不值得——所有 repository trait 都是同步 `&self -> Result<T>`，桌面应用不需要连接池 |
| `tauri-plugin-sql` | JS 导向，不暴露 Rust 端连接，迁移能力弱，与六边形分层不兼容 |
| `diesel` | ORM 过重，schema 规模不需要代码生成和编译期类型映射 |
| `refinery` | 通用迁移框架（支持 Postgres/MySQL），对纯 SQLite 桌面应用过于复杂 |

### 选型理由

- **`bundled` feature**：静态链接 SQLite，零系统依赖，跨平台发布无额外配置
- **同步 API**：匹配现有 trait 签名（`&self -> Result<T>`），无需 `spawn_blocking` 包装
- **`rusqlite_migration`**：利用 SQLite 原生 `PRAGMA user_version`（文件头固定偏移），无需额外迁移跟踪表

---

## 4. 连接管理

```rust
// hmm-infra/src/sqlite/mod.rs

pub fn open_database(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.busy_timeout(Duration::from_secs(5))?;
    run_migrations(&mut conn)?;
    Ok(conn)
}
```

### 连接模型

单 `rusqlite::Connection` 包装在 `Mutex<Connection>` 中，匹配现有 JSON 仓储的 `Mutex<()>` 模式。

| 决策 | 理由 |
|---|---|
| 不用连接池 | 单用户桌面应用，所有仓储调用短暂，无并发读写压力 |
| WAL 模式 | 允许读写并行（未来后台扫描 + UI 查询），无性能劣势 |
| `foreign_keys=ON` | 默认关闭，必须显式启用 |
| `busy_timeout=5000` | 罕见锁竞争时等待而非立即失败 |

### 数据库文件位置

```
{app_data_dir}/hmm.db
```

`app_data_dir` 由 Tauri 的 `app.path().app_data_dir()` 解析：

- Windows: `%APPDATA%\<bundle-identifier>\`
- macOS: `~/Library/Application Support/<bundle-identifier>/`
- Linux: `~/.local/share/<bundle-identifier>/`

与现有 JSON 文件位于同一顶层目录下。

---

## 5. 六边形架构集成

### 层级职责不变

```
hmm-core   — 纯领域模型，零外部依赖，不知道 SQLite
hmm-ports  — 新增 trait（ModMetadataRepository, CategoryRepository, ...）
hmm-app    — 消费 trait，编排业务逻辑
hmm-infra  — 新增 SQLite 实现，与 JSON 实现并存
hmm-tauri  — AppState 接入 + Tauri commands
```

### 依赖变更

| crate | 变更 |
|---|---|
| `hmm-core` | 无变更 |
| `hmm-ports` | 无新外部依赖（只增 trait 定义） |
| `hmm-app` | 无新外部依赖（只消费 trait） |
| `hmm-infra` | 新增 `rusqlite` + `rusqlite_migration` |
| `hmm-tauri` | 无新外部依赖（通过 `AppState` 消费） |

### 模块结构

```
src-tauri/crates/hmm-infra/src/
  sqlite/
    mod.rs                      — open_database() + 迁移入口
    migrations.rs               — 有序 SQL 迁移数组
    mod_metadata_repository.rs  — SqliteModMetadataRepository (T3)
    category_repository.rs      — SqliteCategoryRepository (T4)
    profile_repository.rs       — SqliteProfileRepository (T6)
    binding_repository.rs       — SqliteReplacementBindingRepository (T11)
```

### AppState 接入

```rust
// src-tauri/src/state.rs — AppState::new()

let db_path = app_data_dir.join("hmm.db");
let conn = sqlite::open_database(&db_path)?;
let db = Arc::new(Mutex::new(conn));

// 传入 SQLite 仓储构造函数
let mod_metadata_repo = SqliteModMetadataRepository::new(Arc::clone(&db));
let category_repo = SqliteCategoryRepository::new(Arc::clone(&db));
```

---

## 6. Schema 设计

### Migration 1：Mod 元数据 overlay + 分类（T2/T3/T4 基础）

```sql
CREATE TABLE mod_metadata (
    mod_id        TEXT    PRIMARY KEY NOT NULL,
    display_name  TEXT,
    author        TEXT,
    version       TEXT,
    description   TEXT,
    nexus_mod_id  INTEGER,
    updated_at    INTEGER NOT NULL  -- unix millis
);

CREATE TABLE categories (
    category_id   TEXT    PRIMARY KEY NOT NULL,
    name          TEXT    NOT NULL,
    color         TEXT,                         -- hex color (e.g. "#FF6B6B")
    sort_order    INTEGER NOT NULL DEFAULT 0,
    created_at    INTEGER NOT NULL              -- unix millis
);

CREATE TABLE mod_categories (
    mod_id        TEXT NOT NULL,
    category_id   TEXT NOT NULL,
    PRIMARY KEY (mod_id, category_id),
    FOREIGN KEY (category_id) REFERENCES categories(category_id) ON DELETE CASCADE
);

CREATE INDEX idx_mod_categories_category ON mod_categories(category_id);
```

> **`mod_categories.mod_id` 没有 FK 到 `mod_metadata`**：Mod 的身份来源是 `ModImportResultRepository`（JSON），overlay 表 `mod_metadata` 只在用户编辑时才存在行。分类可以分配给任何已导入的 Mod，不依赖 overlay 行是否存在。`mod_id` 的合法性在应用层校验。

### Migration 2：Profile（T6）

```sql
CREATE TABLE profiles (
    profile_id    TEXT    PRIMARY KEY NOT NULL,
    name          TEXT    NOT NULL,
    description   TEXT    NOT NULL DEFAULT '',
    is_active     INTEGER NOT NULL DEFAULT 0,
    created_at    INTEGER NOT NULL              -- unix millis
);

-- Seed default profile for backward compatibility
INSERT INTO profiles (profile_id, name, is_active, created_at)
    VALUES ('default', 'Default', 1, 0);
```

### Migration 3：ReplacementBinding（T11）

```sql
CREATE TABLE replacement_bindings (
    binding_id    TEXT    PRIMARY KEY NOT NULL,
    mod_id        TEXT    NOT NULL,
    profile_id    TEXT    NOT NULL,
    source_path   TEXT    NOT NULL,
    target_id     TEXT    NOT NULL,
    created_at    INTEGER NOT NULL,
    FOREIGN KEY (profile_id) REFERENCES profiles(profile_id) ON DELETE CASCADE
);

CREATE INDEX idx_bindings_mod_profile
    ON replacement_bindings(mod_id, profile_id);
CREATE INDEX idx_bindings_target
    ON replacement_bindings(target_id);
```

---

## 7. 迁移策略

### 工具

使用 `rusqlite_migration` 管理迁移。

```rust
use rusqlite_migration::{Migrations, M};

static MIGRATIONS: Migrations<'static> = Migrations::from_slice(&[
    M::up(include_str!("migrations/001_metadata_categories.sql")),
    M::up(include_str!("migrations/002_profiles.sql")),
    M::up(include_str!("migrations/003_replacement_bindings.sql")),
]);
```

### 规则

| 规则 | 说明 |
|---|---|
| 前向迁移 | 不写 down migration；回滚 = 删除 `hmm.db`（overlay 数据丢失可接受，安装链路数据在 JSON 中不受影响） |
| 启动时执行 | `open_database()` 在返回 `Connection` 之前运行 `MIGRATIONS.to_latest()` |
| 版本追踪 | `PRAGMA user_version`（SQLite 文件头固定偏移），无额外表 |
| SQL 嵌入 | 使用 `include_str!()` 嵌入 `.sql` 文件，编译时固化，运行时无文件解析 |
| 首次启动 | 无数据库文件时，SQLite 自动创建并依次执行全部迁移 |

---

## 8. 实施路线图

每个阶段对应一个独立 PR，保持小切片可验证。

### PR 1：SQLite 基础设施（T2-infra）

**不新增任何 port trait，不改变现有行为。**

- `hmm-infra/Cargo.toml` 添加 `rusqlite` + `rusqlite_migration`
- 新建 `hmm-infra/src/sqlite/mod.rs`：`open_database()` + pragma + 迁移
- 新建 `hmm-infra/src/sqlite/migrations.rs`：Migration 1 SQL
- `src-tauri/src/state.rs`：`AppState::new()` 中创建 `Connection` 并存储为 `Arc<Mutex<Connection>>`
- 单元测试：open → migrate → verify schema（`:memory:` + `tempfile`）

### PR 2：Mod 元数据 overlay（T3）

- `hmm-core`：定义 `ModMetadataOverlay`
- `hmm-ports`：新增 `ModMetadataRepository` trait（`get`/`save`/`delete` overlay）
- `hmm-infra/src/sqlite/mod_metadata_repository.rs`：SQLite 实现
- `hmm-app`：`ModMetadataService` 合并 import 快照 + overlay
- `hmm-tauri`：`update_mod_metadata` / `delete_mod_metadata` command
- 前端 typed API

### PR 3：分类标签 CRUD（T4）

- `hmm-core`：`Category` 领域模型
- `hmm-ports`：`CategoryRepository` trait（CRUD + assign/unassign mod）
- `hmm-infra/src/sqlite/category_repository.rs`：SQLite 实现
- `hmm-app` + `hmm-tauri` CRUD commands
- 前端 typed API + `get_mod_library` 返回真实分类

### PR 4：Profile 管理（T6）

- `hmm-infra/src/sqlite/migrations.rs`：追加 Migration 2
- `hmm-core`：`Profile` 领域模型
- `hmm-ports`：`ProfileRepository` trait
- `hmm-infra/src/sqlite/profile_repository.rs`：SQLite 实现
- 前端 Profile 列表/创建/切换/删除
- 替换前端硬编码 `DEFAULT_INSTALL_PROFILE_ID = "default"`

### PR 5：ReplacementBinding（T11）

- `hmm-infra/src/sqlite/migrations.rs`：追加 Migration 3
- `hmm-ports`：`ReplacementBindingRepository` trait
- `hmm-infra/src/sqlite/binding_repository.rs`：SQLite 实现
- 集成到 ARMOR_RETARGET 全链路

---

## 9. 测试策略

| 层级 | 方式 |
|---|---|
| SQLite 仓储单元测试 | `:memory:` 数据库 + `MIGRATIONS.to_latest()` |
| `open_database` 集成测试 | `tempfile` 目录创建真实 `.db` 文件 |
| 迁移幂等性测试 | 多次 `to_latest()` 不 panic |
| Application 层测试 | 继续 mock port traits，不涉及 SQLite |
| 现有 JSON 仓储测试 | 不变 |

---

## 附录：关键设计决策索引

| 决策 | 理由 |
|---|---|
| `Mutex<Connection>` 不用连接池 | 单用户桌面应用，短暂调用，匹配 JSON 仓储现有模式 |
| `mod_categories.mod_id` 无 FK 到 `mod_metadata` | Mod 身份源是 JSON `ModImportResultRepository`，overlay 行非必需 |
| 不迁移 JSON 仓储到 SQLite | 安装链路已验证稳定 22 个 PR，迁移无收益有风险 |
| SQL 嵌入 Rust 源码 | `include_str!()` + `rusqlite_migration` 设计模式，编译时固化 |
| 前向迁移，无 down migration | 桌面应用回滚策略 = 删库重建，overlay 数据丢失可接受 |
| `rusqlite` 而非 `sqlx` | 所有 trait 是同步签名，异步开销不值得 |
