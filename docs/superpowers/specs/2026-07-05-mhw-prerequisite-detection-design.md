# MHW 前置依赖检测设计

## 背景

当前仓库已经具备：

- MHW:I 游戏目录配置与校验。
- 导入 Mod 的 metadata 解析与只读 dependency graph。
- Profile 保存目录、存档备份和安装链路的基础能力。

但“前置依赖”仍停留在架构占位阶段。现在后端能展示某个导入包声明了哪些 `dependencies`，却不能回答“当前已配置的 MHW:I 游戏环境里，这些前置是否真的安装好了”。

本设计先覆盖一个很小但真实有用的切片：只检测两个已知 MHW:I 前置，并把结果展示到游戏目录 / 设置页的全局环境自检中。

当前确认的两个前置：

- `Stracker's Loader`
- `CRCBypass`

## 目标

- 在当前已配置的 MHW:I 游戏目录上执行只读前置检测。
- 只覆盖 `Stracker's Loader` 和 `CRCBypass` 两个已知前置。
- 通过“文件存在 + 已知 hash + `loader-config.json` 关键字段”给出结构化状态。
- 将检测结果接入游戏目录 / 设置页的全局环境自检，而不是安装 preflight。
- 让前置规则脱离 Rust 源码常量，改为默认规则文件驱动。
- 运行时不依赖任何用户本地测试目录，例如 `D:\G\mh\mod-config`。

## 非目标

- 本轮不实现安装前 dependency/preflight 阻断。
- 本轮不实现通用前置规则框架，只覆盖两个已知前置。
- 本轮不支持用户自定义或远程更新前置规则。
- 本轮不把前置状态写入 manifest、audit、profile 或数据库。
- 本轮不根据压缩包、解压缓存或 `mod-config` 目录推断“已安装”事实。
- 本轮不做目录写入、前置自动安装、自动修复或自动下载。

## 方案比较

### 方案 A：只检查文件存在

只判断关键文件是否位于游戏根目录或 `nativePC/plugins` 下。

优点是实现简单，缺点是可靠性不足：错误版本、损坏文件或被其他包替换过的 DLL 也会被误判为“已安装”。

结论：不采用。

### 方案 B：文件存在 + 已知 hash + 配置校验

只读检查当前游戏目录中的关键文件；DLL 通过已知 `SHA-256` 集合判断是否属于已验证版本；`loader-config.json` 只检查关键字段 `enablePluginLoader = true`。

优点是状态可解释，且符合现有架构文档中“前置依赖检测应数据驱动、可基于文件存在和 hash”的方向。

结论：推荐采用。

### 方案 C：把用户本地 `mod-config` 解压目录当成运行时基准

运行时读取 `mod-config` 下的 zip 或解压目录，拿它们和游戏目录比对。

优点是短期直观，缺点是直接把用户测试环境变成运行时依赖，不可移植，也不适合作为正式产品能力。

结论：不采用。

## 推荐设计

采用方案 B。

运行时事实来源只有“当前已配置的游戏目录里的实际文件”。`mod-config` 目录只允许作为一次性的样本来源，用于整理默认规则文件中的已知 hash；应用运行时绝不访问该目录。

总体流程：

```text
前端进入游戏目录 / 设置页
  -> get_game_prerequisite_status("mhw")
  -> hmm-app 读取已保存的 game instance
  -> 若未配置目录，返回 not_configured
  -> 若目录当前失效，返回 game_directory_invalid
  -> 创建只读 probe
  -> MHW adapter 读取前置规则并检测
  -> 返回结构化结果给前端
  -> 前端展示“缺失 / 配置错误 / 已验证 / 未验证”状态
```

关键边界：

- 前端只消费结构化结果，不计算路径，不做 hash，不解析 `loader-config.json`。
- `hmm-core` 不知道 `Stracker's Loader`、`CRCBypass` 或 `nativePC/plugins`。
- MHW 专属路径和规则只留在 `hmm-games-mhw`。
- 检测是只读行为，不创建 task，不发送 progress event，不写入游戏目录。

## 检测对象与状态模型

### 前置对象

本轮只覆盖：

#### `stracker_loader`

必需文件：

- `dinput8.dll`
- `loader.dll`
- `loader-config.json`
- `nativePC/plugins/MonsterLoader.dll`
- `nativePC/plugins/QuestLoader.dll`

#### `crc_bypass`

必需文件：

- `nativePC/plugins/!CRCBypass.dll`

### `loader-config.json` 规则

本轮只检查：

- 文件存在
- JSON 可解析
- `enablePluginLoader` 必须为 `true`

以下字段不作为硬判据：

- `logfile`
- `logcmd`
- `logLevel`
- `outputEveryPath`

原因：这些更像用户可调偏好，不应该因为日志设置不同就把前置判成未安装。

### 前置状态

每个前置返回以下四档之一：

- `missing`
- `misconfigured`
- `installed_verified`
- `installed_unverified`

判定优先级：

```text
missing
  > misconfigured
  > installed_unverified / installed_verified
```

解释：

- 缺任一必需文件，直接是 `missing`。
- 文件齐全但 `loader-config.json` 解析失败或关键字段不对，是 `misconfigured`。
- 只有“文件齐全且配置合法”后，才进入 `verified / unverified` 的 hash 分流。

### `installed_unverified` 语义

`installed_unverified` 表示：

- 关键文件存在；
- `loader-config.json` 合法；
- 但一个或多个 DLL 的 hash 不在当前规则文件的已知集合里。

该状态只作为 warning，不在本轮视为硬阻断。

## 规则文件设计

### 设计目标

- hash 规则不硬编码在 Rust 代码里。
- 默认规则随应用分发。
- 运行时读取本地配置目录中的规则文件。
- 当前版本先不支持用户覆盖，但文件形态必须为未来覆盖能力留出口。

### 文件落点

仓库内默认模板：

- `src-tauri/crates/hmm-games-mhw/data/mhw-prerequisites.default.json`

运行时实际规则：

- app config 目录下的 `prerequisite-rules/mhw.json`

### 启动 / 首次读取行为

1. 读取前置规则时，先查找本地 `prerequisite-rules/mhw.json`。
2. 若不存在，则把仓库内默认模板复制到本地 config 目录。
3. 后续运行时只读取本地 `mhw.json`。

这样后续前置更新时，只替换本地规则文件即可；无需修改 Rust 代码或重新编译。

### 规则文件结构

建议结构：

```json
{
  "version": 1,
  "gameId": "mhw",
  "prerequisites": [
    {
      "id": "stracker_loader",
      "displayName": "Stracker's Loader",
      "requiredFiles": [
        "dinput8.dll",
        "loader.dll",
        "loader-config.json",
        "nativePC/plugins/MonsterLoader.dll",
        "nativePC/plugins/QuestLoader.dll"
      ],
      "signatureFiles": [
        {
          "path": "dinput8.dll",
          "sha256": [
            "6E38BAFF0BDDC5014046E3BA5A733814F95F65D5CA67E2FB15D18C5106D4E059"
          ]
        }
      ],
      "jsonChecks": [
        {
          "path": "loader-config.json",
          "requiredBooleanFields": {
            "enablePluginLoader": true
          }
        }
      ]
    }
  ]
}
```

`sha256` 使用数组而不是单值。这样后续前置升版本时，只需要给对应文件追加新 hash，不需要改判定逻辑。

### 默认样本 hash

当前确认的默认样本 `SHA-256`：

#### Stracker's Loader

- `dinput8.dll`
  - `6E38BAFF0BDDC5014046E3BA5A733814F95F65D5CA67E2FB15D18C5106D4E059`
- `loader.dll`
  - `17EC93D9D57809E4968666961CAF996F7D819C05B280FBB6D444B95920A801EE`
- `nativePC/plugins/MonsterLoader.dll`
  - `F307FD30C30D708980990062C0344C0034FB4363BB6FB85D8217E0134CEA7D9B`
- `nativePC/plugins/QuestLoader.dll`
  - `97380A19C12822C318EBC7EF09DF601823CBF33EC674E1AEE9F8A690D5422C08`

#### CRCBypass

- `nativePC/plugins/!CRCBypass.dll`
  - `6F5EC7D28B9EE4CFBB341B778B710F3646CAEBA1A213FF0DB85281E1A972D058`

这些值只作为默认规则起点，不代表以后只能接受这些版本。

## 后端分层

### Ports

`hmm-ports` 需要补充两类能力：

1. `GameAdapter` 的只读前置检测入口
2. 更强一点的只读 probe 能力

建议在 `game_setup` 相关端口中增加：

- 前置检测结果模型：
  - `GamePrerequisiteReport`
  - `GamePrerequisiteItem`
  - `GamePrerequisiteStatus`
  - `GamePrerequisiteIssue`
- 只读 probe 能力：
  - 判断相对路径文件是否存在
  - 读取文本文件
  - 计算指定文件的 `SHA-256`

这些类型必须保持游戏无关；不能在 ports 里出现 `nativePC`、`Stracker` 或 `CRCBypass` 文本。

### MHW Adapter

`hmm-games-mhw` 负责：

- 加载 / 初始化 MHW 默认前置规则文件
- 校验规则文件 schema
- 根据规则执行只读检测
- 把 MHW 专属相对路径、hash 和 `loader-config.json` 规则留在 adapter 内

建议新增模块：

- `src-tauri/crates/hmm-games-mhw/src/prerequisites.rs`
- `src-tauri/crates/hmm-games-mhw/data/mhw-prerequisites.default.json`

### Infra

`hmm-infra` 负责：

- 为现有游戏目录 probe 提供“读文本 + 算 hash”实现
- 提供前置规则文件的本地 JSON 读写仓储
- 首次缺失时把 bundled 默认模板复制到本地 config 目录

规则文件仓储风格应与现有 `src-tauri/crates/hmm-infra/src/app_settings_repository.rs` 中的 `JsonAppSettingsRepository` 一致：

- 带 `version`
- 非法 JSON / schema 视为损坏
- 写入使用临时文件 + rename

### App

`GameSetupService` 新增一个只读查询，例如：

- `get_prerequisite_status(game_id)`

职责：

- 读取当前已保存的 `GameInstance`
- 未配置目录时返回 `not_configured`
- 已配置但目录当前失效时返回 `game_directory_invalid`
- 目录有效时调用 adapter 前置检测

前端只传 `gameId`，不传目录路径。

### Tauri

新增窄 command，例如：

- `get_game_prerequisite_status(gameId)`

DTO 只返回：

- 前置 id
- 展示名
- 状态
- 稳定 reason code
- 相对路径明细

禁止返回：

- 游戏绝对路径
- 用户名
- `mod-config` 路径
- 任意本地私有目录

## UI 设计

第一版先接到游戏目录 / 设置页的全局环境自检，不做安装前阻断。

展示两个前置项：

- `Stracker's Loader`
- `CRCBypass`

每项显示：

- 名称
- 状态徽标
- 一句摘要
- 可展开明细

建议文案：

- `missing`：缺少必需文件
- `misconfigured`：配置不正确
- `installed_verified`：已安装，版本已验证
- `installed_unverified`：已安装，但版本未验证

明细示例：

- 缺失的相对路径
- 未验证的相对路径
- `loader-config.json` 的配置错误

`installed_unverified` 必须展示为 warning，而不是 hard error。

## 错误处理

### 规则文件损坏

若本地规则文件存在但 JSON / schema 非法，前端应看到“前置规则不可用”类错误，而不是被误导为“前置缺失”。

这是“规则源损坏”，不是“游戏环境缺文件”。

### 未配置或失效目录

- 未配置游戏目录：返回 `not_configured`
- 已配置目录但当前已失效：返回 `game_directory_invalid`

这种状态下不执行前置检测。

### 读取失败

若个别文件读取失败、hash 计算失败或 JSON 解析失败，应转换成稳定错误码并保留到对应前置项的 issue 中，而不是把错误文本直接透传给前端。

## 测试策略

### `hmm-games-mhw`

至少覆盖：

- 两个前置全部命中时得到 `installed_verified`
- 缺任一必需文件时得到 `missing`
- `loader-config.json` 缺少 `enablePluginLoader = true` 时得到 `misconfigured`
- 文件存在但 hash 不在规则集合里时得到 `installed_unverified`
- 本地规则文件缺失时会从默认模板初始化
- 规则文件损坏时返回“规则不可用”类错误

### `hmm-app`

至少覆盖：

- 游戏目录未配置时返回 `not_configured`
- 游戏目录失效时返回 `game_directory_invalid`
- 目录有效时会调用 adapter 前置检测并返回结果

### Tauri / DTO

至少覆盖：

- 状态和 reason code 序列化稳定
- DTO 不包含绝对路径
- 明细只包含相对路径

### Frontend

至少覆盖：

- 设置页 / 游戏目录页展示两个前置项
- 四种状态都有对应显示
- `installed_unverified` 呈现为 warning
- UI 不暴露真实本地路径

## 验收标准

- 当前已配置的 MHW:I 游戏目录可返回两个前置的结构化状态。
- 规则文件不依赖 Rust 源码硬编码 hash。
- 运行时不访问 `mod-config` 或其他测试目录。
- `loader-config.json` 至少校验 `enablePluginLoader = true`。
- 未知 hash 记为 `installed_unverified`，给 warning，不阻断。
- 前端能在游戏目录 / 设置页展示结果，并且不泄露绝对路径。
