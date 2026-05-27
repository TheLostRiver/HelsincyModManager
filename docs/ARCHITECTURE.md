# 架构设计

## 项目定位

Helsincy Mod Manager 不是一个简单的压缩包解压工具，而是一个本地游戏 Mod 管理平台。

第一阶段会以《怪物猎人：世界 冰原》为首个支持目标，但整体架构必须允许后续扩展到《怪物猎人：崛起》《怪物猎人：荒野》以及其他具有类似资源替换机制的游戏。

第一版以 Windows 可用为重点，同时通过平台抽象保留 Linux / Steam Deck 支持空间。

## 核心原则

- UI 和核心业务逻辑分离。
- 应用用例依赖 trait / interface，而不是依赖具体实现。
- 游戏差异全部收敛到游戏适配器。
- 安装必须基于安装计划和安装清单，不能随手复制文件。
- 面向玩家的规则尽量数据驱动。
- 重任务必须后台执行，并通过事件向前端汇报进度。
- 同一个游戏实例的写入操作必须串行。
- 所有破坏性操作都必须可回滚或可恢复。

## 总体分层

```text
前端 UI
  React + TypeScript
  负责展示、交互、筛选、弹窗、进度展示

Tauri Commands
  前端与 Rust 后端之间的薄边界
  负责参数校验和 DTO 转换

Application 应用层
  导入 Mod、安装 Mod、禁用 Mod、备份存档、启动游戏等用例

Domain 领域层
  Mod、Game、Profile、InstallPlan、Conflict、Manifest、Dependency、ReplacementTarget

Ports / Traits 接口层
  文件系统、压缩包、数据库仓储、游戏适配器、启动器、任务系统

Infrastructure 基础设施层
  SQLite、真实文件系统、压缩工具、hash、Steam 库扫描、平台 API

Game Adapters 游戏适配器
  首先支持 Monster Hunter: World - Iceborne
  后续扩展 Monster Hunter Rise / Wilds
```

## Rust Workspace 规划

```text
src-tauri/
  crates/
    hmm-core/          # 纯领域模型和规则，不接触真实系统 API
    hmm-ports/         # 应用层依赖的 traits/interfaces
    hmm-app/           # 应用用例和流程编排
    hmm-infra/         # SQLite、文件系统、压缩包、hash、Steam 扫描
    hmm-games-mhw/     # MHW:I 适配器和游戏规则
    hmm-tauri/         # Tauri state、commands、events、应用启动
```

前端按功能拆分：

```text
src/
  features/
    mods/
    categories/
    profiles/
    conflicts/
    backups/
    games/
    settings/
  shared/
    api/
    components/
    state/
    types/
```

## 主要模块

### 游戏发现

游戏目录识别需要支持多种策略：

- 扫描 Steam library
- 扫描正在运行的进程
- 玩家手动选择目录

发现模块返回 `GameInstance`，不能假设游戏只有一个固定路径。

### 游戏启动

启动逻辑由平台和游戏适配器决定：

- 优先通过 Steam 协议启动
- 必要时直接启动游戏 exe
- 后续 Linux / Steam Deck 通过独立平台实现处理

启动前可以检查：

- 是否缺少必要前置
- 当前 profile 是否存在冲突
- 是否有未完成的安装任务
- 游戏目录是否仍然有效

### Mod 导入流水线

导入压缩包不能直接安装，必须先经过安全流水线：

```text
选择压缩包
检查压缩包信息
拒绝危险路径
解压到沙盒缓存目录
分析文件结构
提取并校验预览图
推断 Mod 类型
生成元数据
生成候选安装计划
```

导入器必须防御：

- `../` 路径穿越
- 绝对路径
- 压缩包炸弹
- 不支持或可疑的文件类型
- 伪装图片扩展名
- 大小写不敏感平台上的路径冲突

### 包分析器

包分析器识别 Mod 内容，例如：

- `nativePC` 文件
- 游戏根目录 DLL
- exe 或辅助工具
- INI / JSON / config 文件
- readme 文件
- 预览图片
- 外观、武器、语音替换相关的资源编号

包分析器输出结构化信息，不能把安装规则塞进前端。

### 分类和标签

分类和标签必须支持多对多关系。

默认分类可以包括：

- 外观
- 主角外观
- NPC 外观
- 随从外观
- 武器替换
- 语音替换
- 功能性 Mod
- 武器特效
- 前置
- 工具

玩家必须可以创建自定义分类，并把一个 Mod 放到多个分类或标签下。

### 前置依赖检查

很多怪猎 Mod 需要前置文件或 loader。依赖检查必须数据驱动。

依赖规则的大致形态：

```text
DependencyRule
  id
  display_name
  severity
  detection_rules
```

检测方式可以包括：

- 游戏根目录存在某文件
- `nativePC` 下存在某文件
- 文件 hash 匹配已知值
- 安装清单中存在某个前置 Mod

缺少必需前置时，安装应被阻止或给出明确警告，具体行为由严重级别决定。

### 替换目标映射

外观、武器、语音 Mod 经常不是单纯“安装文件”，而是把自定义资源覆盖到官方资源槽位上。管理器必须把这种关系建模为一等概念。

核心模型：

```text
ReplacementTarget
  官方游戏资源槽位
  例如：某套外观、某个部位、某把武器、某个语音槽位

ReplacementBinding
  玩家选择的“Mod 资源 -> 官方目标”的绑定关系

RetargetPlan
  为了把 Mod 重定向到目标槽位，需要在 staging 目录执行的改写计划
```

外观替换需要支持：

- 可拆分外观：头、胸、手、腰、脚
- 固定整套外观：联动整套、不可拆分整套
- 未来高级拆分或转换流程，通过插件式 transformer 扩展

重要规则：

- 原始导入的 Mod 包永远只读。
- 重定向只发生在 staging 目录。
- 安装清单记录玩家选择的替换绑定。
- 冲突检测基于最终目标路径，而不是原始压缩包路径。
- 玩家切换目标时，本质上是卸载旧绑定，再安装新绑定。

### 安装计划

安装前必须先生成 `InstallPlan`。

计划动作示例：

```text
CopyFile
CreateDirectory
BackupExistingFile
RemoveFile
WriteManifest
```

安装计划负责：

- 将包内容转换成游戏目标路径
- 应用替换目标绑定
- 检测冲突
- 检查前置依赖
- 估算任务量，用于进度展示

### 安装执行器

安装执行器负责真正修改游戏目录。

要求：

- 覆盖文件前必须备份。
- 安装完成后必须写安装清单。
- 失败时尽可能回滚。
- 同一个游戏实例的写入必须串行。
- 记录足够状态，用于崩溃或强制关闭后的恢复扫描。

### 存档备份服务

存档备份模块独立于 Mod 安装模块。

必备能力：

- 手动备份
- 自动备份
- 玩家自选备份目录
- 未选择时使用默认备份目录
- 自动备份时间间隔可配置
- 按数量、时间或空间占用设置保留策略
- 备份清单和 hash 校验

默认备份目录应位于应用数据目录下，而不是游戏目录里。

### 任务管理器

长耗时操作必须作为后台任务执行：

- 压缩包解压
- 包扫描
- hash 计算
- 冲突分析
- 安装计划生成
- 安装执行
- 存档备份压缩

前端通过 Tauri command 启动任务，通过事件接收进度。

## 并发模型

并发原则：

```text
读取和准备工作可以并行。
同一个游戏实例的写入必须串行。
```

建议的任务分组：

- CPU pool：hash 和冲突分析
- IO pool：扫描、解压、复制前准备
- Game write queue：每个游戏实例一个串行写入队列
- Database transaction：短事务、明确写入边界
- Event bus：进度和日志事件

采用两阶段执行：

```text
Prepare 阶段
  解压、hash、分析、依赖检查、生成计划
  可并行、可取消、不碰游戏目录

Commit 阶段
  获取游戏写锁
  重新校验当前状态
  备份、复制、删除、写清单
  短时间串行、可恢复
```

不要在持有游戏写锁时做长时间解压或 hash。

## 数据存储

SQLite 存储用户数据和运行状态：

- 游戏实例
- 已导入 Mod
- 分类和标签
- Profile
- 替换绑定
- 安装清单
- 备份历史
- 用户设置

JSON 或 TOML 存储偏规则的数据：

- 默认分类
- 官方替换目标 catalog
- 前置依赖规则
- 存档路径规则
- Mod 类型识别规则
- 备份策略默认值
- 预览图大小、压缩包大小等限制

## 关键领域模型

```text
GameDefinition
  id
  display_name
  adapter_id
  supported_platforms

GameInstance
  id
  game_id
  install_path
  platform
  launcher

ModEntry
  id
  name
  version
  package_ref
  categories
  tags
  dependencies

ModPackage
  id
  archive_path
  extracted_cache_path
  detected_type
  files
  preview_image
  metadata

ReplacementTarget
  id
  game_id
  target_type
  internal_id
  display_name
  part
  is_full_body

ReplacementBinding
  id
  mod_id
  profile_id
  source_asset
  target_id

InstallPlan
  id
  actions
  conflicts
  dependency_result
  replacement_bindings

InstallManifest
  id
  mod_id
  profile_id
  installed_files
  backups
  hashes
  replacement_bindings
```

## 关键 Traits

```rust
pub trait GameAdapter {
    fn game_id(&self) -> GameId;
    fn detect_instances(&self) -> Result<Vec<GameInstance>>;
    fn analyze_package(&self, package: &ModPackage) -> Result<GamePackageInfo>;
    fn build_install_plan(&self, request: InstallRequest) -> Result<InstallPlan>;
    fn dependency_rules(&self) -> Result<Vec<DependencyRule>>;
    fn replacement_catalog(&self) -> Result<Vec<ReplacementTarget>>;
}

pub trait FileSystem {
    fn exists(&self, path: &Path) -> bool;
    fn copy_file(&self, from: &Path, to: &Path) -> Result<()>;
    fn remove_file(&self, path: &Path) -> Result<()>;
    fn create_dir_all(&self, path: &Path) -> Result<()>;
}

pub trait ArchiveExtractor {
    fn inspect(&self, archive: &Path) -> Result<ArchiveInfo>;
    fn extract_to(&self, archive: &Path, target: &Path) -> Result<()>;
}

pub trait ModRepository {
    fn save(&self, mod_entry: &ModEntry) -> Result<()>;
    fn get(&self, id: ModId) -> Result<Option<ModEntry>>;
}
```

## MVP 范围

第一版应包含：

- MHW:I 游戏目录识别和手动选择
- Mod 压缩包导入和安全校验
- 预览图提取和校验
- 分类和标签管理
- 基础前置依赖检查
- 安装 / 卸载 / 安装清单
- 基于最终路径的冲突检测
- 手动存档备份
- 一键启动游戏

## 后续范围

MVP 之后再加入：

- 外观、武器、语音替换目标选择
- Profile
- 自动存档备份
- 高级回滚和恢复 UI
- 任务队列 UI
- Linux / Steam Deck 实验性打包和社区测试
