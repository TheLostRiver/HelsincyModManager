# 导入压缩格式支持设计

> 状态（2026-09-08）：**A、B 已合并**（PR #370 / #371），C 进行中。切片顺序
> **A 失败档位 → B 安全外壳与格式解耦 → C rar → D 7z**；`.tar` 家族（T21-E）本轮延后。
> C 再分三步：**C1 `hmm-unrar-sys`（vendor ＋ build.rs ＋ FFI，不接链路）→
> C2 测试语料生成器 → C3 接入外壳与判别**。这么分是因为 C1 的风险
> （48 个 C++ 文件双平台能不能编、结构体布局对不对）与 C3 的风险（安全语义）
> 性质完全不同，混在一个 diff 里没法连贯 review。
> 缺陷面由 #348 立案。维护者已于 2026-09-08 拍板：rar 做，且自行 vendor 官方 UnRAR 源码
> 而非使用现成 crate。任务状态与切片清单见 [任务总纲](../TODO.md) 的 T21，本文件是其权威设计。

## 背景

HMM 目前只支持 zip。导入 `.rar` 时玩家看到的是：

| 位置 | 文案 |
| --- | --- |
| 阶段 | 「安全解包失败」 |
| 提示 | 「导入失败，请检查压缩包后重试」 |

**压缩包完好无损。** 触发它的是一个真实素材：55 MB、7339 个文件的画面整合包，转成 zip 后导入与
安装链路的每一道门全部通过——内容没问题，卡的只是容器格式（#348）。

这与 #336 / #345 / #346 是同一类缺陷：**包本身是好的，错误信息把玩家指向错误的方向。**

### 成因不是「缺一个档位」，是链路上没有传递原因的通道

排查后确认，两头都堵死了：

```rust
// hmm-app/src/mod_import.rs:1150
fn failed_event(task_id: &str) -> TaskProgressEvent {
    ...
    event.error = Some(MOD_IMPORT_PREPARE_FAILED_ERROR.to_owned());  // 常量，所有解包失败共用
}
```

```ts
// src/features/mods/modImportTaskState.ts:141
if (event.status === "failed") {
  return { status: "failed", taskId: event.taskId, phase: event.phase, messageKind: "retry-hint" };
  //                                                                   ↑ 完全没读 event.error
}
```

后端每一种解包失败都发同一个常量码；前端在失败分支里根本不看 `error`。所以这不是「少了一档」，
而是**「原因」这个概念在这条链上不存在**。修好之后受益的不止 rar——此后每一种解包失败都有地方落。

### 现状清单（已核实）

| 事实 | 位置 |
| --- | --- |
| 生产环境唯一的解包实现 | `hmm-infra/src/mod_import.rs:173` —— 全仓仅此一处 `impl ModImportPackagePreparer`，其余都是测试 fake |
| 解包入口写死 zip | 同文件 `:635` `zip::ZipArchive::new(…)`；依赖 `zip` 2.4.2，无 rar / 7z |
| 端口返回不带语义 | `hmm-ports/src/mod_import.rs:68` 的 `prepare_package` 返回 `anyhow::Result<PreparedModPackage>` |
| 文件对话框只列 zip | `src/features/mods/ModImportAction.tsx:320` `extensions: ["zip"]`（Windows 上是显示过滤器，不是硬校验） |
| 失败档位表 | `src/features/mods/modImportTaskState.ts:42-50`，8 档，无「格式不支持」 |

### 现有安全链（全部长在 zip reader 里）

| 门禁 | 位置 | 判定时机 |
| --- | --- | --- |
| 条目数上限 64K | `:26`、`:637` | **解包前**，从 zip 中央目录预先拿到 `archive.len()` |
| 单文件上限 4 GiB | `:27`、`:646` | 逐条目，读**声明**大小 `entry.size()` |
| 总解压上限 16 GiB | `:28`、`:658` | 逐条目累加**声明**大小 |
| `..` 逃逸拒绝 | `:781` `safe_zip_entry_path` | 逐条目 |
| symlink 条目拒绝 | `:749` `reject_symlink_entry` | 逐条目 |
| 大小写重名冲突拒绝 | `:640` | 逐条目 |
| task-scoped sandbox | `cap_std::fs::Dir` | 全程 |

**每加一个格式复制一遍这七条，早晚有一份漏掉其中一条。** 这是 T21-B 存在的唯一理由。

## 设计结论

1. **失败原因必须成为契约的一部分**，不能靠 `anyhow` 文本或 downcast 推断。
2. **容器判别只用于解释失败，绝不用于拦截输入**——先尝试打开，失败后才嗅探。
3. **安全外壳必须与格式解耦**，格式实现只提供条目迭代能力，七条门禁由外壳统一施加。
4. **第三方解压器不得自行落盘**，落盘位置一律由外壳决定。这既是架构一致性要求，
   也是 rar 的核心安全缓解措施。
5. **配额挂在字节流层，不挂元数据层**，且超限要能立即中止而不是读完再判。

## 目标

- 玩家导入不支持的容器时，得到**可操作**的提示，而不是「请检查压缩包后重试」。
- 支持 rar 与 7z 的导入，安全属性与 zip 完全一致。
- 新增格式不新增安全代码路径。

## 非目标

- **不做压缩**。只解压。（这同时是 UnRAR 许可的硬性要求，见「许可义务」。）
- 不改 `ModImportPackagePreparer` 之上的任何东西：任务模型、脱敏、manifest、存储、安装链路一律不动。
- 不因为支持了新格式就改变「原始包只读」「解包只进 sandbox」这两条。
- 不做 `.tar` 家族（本轮延后，理由与分析见 [任务总纲](../TODO.md) T21-E）。
- 不做加密包的密码交互；加密只需落到明确档位。

## 已定的取舍

### 取舍一：rar 走自行 vendor，不用现成 crate（2026-09-08 拍板）

| | A：用 `unrar` / `unrar_sys` crate | **B：自行 vendor 官方源码 ＋ 写 FFI（选定）** |
| --- | --- | --- |
| vendor 版本 | 7.21 beta 1（2026-03-22） | **7.23 正式版（2026-06-27）**，自主可升 |
| 安全更新 | 受制于上游 fork 的发布节奏 | 自己控 |
| 落盘控制 | 整条目入内存后由我们写 | 回调流式收字节，**unrar 全程不碰文件系统** |
| 配额语义 | 锁死在「整条目入内存」 | 流式 ＋ 可中止，与其他格式统一 |
| 成本 | 低 | `build.rs`（`cc` 编译 48 个 `.cpp`，Windows 另加 3 个）＋ 6 个 `extern "C"` ＋ 2 个 struct ＋ 1 个回调 |
| 仓库影响 | 无 | ＋1.3 MB / 150 个文件；单文件最大 58 KB，远低于 256 KB 门禁 |

决定性理由是「落盘控制」那一行：用 crate 只能做到「整条目入内存再由我们写」，
用 C API 能做到「流式收字节 ＋ 可中止 ＋ unrar 全程不写盘」。**只有后者能让 T21-B 的外壳对 rar
真正生效**，也才能消掉威胁模型里那一整类 symlink 绕过风险。

绑定 crate 自身是 MIT OR Apache-2.0；受 UnRAR 许可约束的只有 vendor 进来的 C++ 源码。两条路在
许可上没有差别，差别全在控制权。

### 取舍二：一处已被推翻的判断，留档

2026-09-07 曾根据 `unrar` crate 的 `read()` 返回 `Vec<u8>`，写下过
**「rar 只能整条目入内存，配额必须按声明大小预先硬拒」**，并据此要求安全外壳同时容纳两种互斥的
配额语义。

**该判断已被官方源码推翻，作废。** 核对结果：

- `dll.hpp:131` 定义 `UNRARCALLBACK`，`dll.hpp:166` 的消息枚举含 `UCM_PROCESSDATA`
- `rdwrfn.cpp:162`：解压器的输出函数 `UnpWrite` **每产出一块就带 `(Addr, Count)` 回调一次**
- 同处：回调**返回 `-1` 即中止解压**（`RARX_USERBREAK`）

即 C API 本身是流式且可中止的，「整条目入内存」是那个 crate 的抽象，不是格式或 API 的限制。
外壳因此**只需要一种**配额语义。

留这一段是因为：这是「把某个库的抽象当成底层能力」的典型错误，而它差点让外壳的接口为一个
不存在的约束扭曲。

## 信任边界与威胁模型

### 输入是不可信的

导入链路处理的是**玩家自己从互联网下载的任意压缩包**。这不是内部数据，也不是我们生成的数据。
唯一可信的是：文件路径是玩家自己选的。

当前解包链路从头到尾是 Rust（`zip` crate ＋ `cap_std` 沙箱），内存安全类问题基本被语言挡掉。
**接入 unrar 会把一个 C++ 解析器放进应用进程去解析不可信输入**，这是本设计引入的唯一新增
攻击面，必须显式论证。

### unrar 的已知漏洞史（NVD）

| CVE | 年 | 等级 | 类型 |
| --- | --- | --- | --- |
| CVE-2012-6706 | 2012 | CRITICAL | VMSF_DELTA 内存损坏 → 任意代码执行 |
| CVE-2017-12940 / 12941 / 12942 | 2017 | CRITICAL | 越界读、缓冲区溢出 |
| CVE-2017-12938 | 2017 | HIGH | symlink 链绕过目录遍历防护 |
| CVE-2022-30333 | 2022 | HIGH | 目录遍历，写到目标目录之外 |
| CVE-2022-48579 | 2023 | HIGH | symlink 链绕过 |
| CVE-2026-24857 | 2026 | CRITICAL | **内嵌** unrar 的 PPM LZ 解码器堆溢出 |

最后一条与本设计的形态完全相同（下游项目内嵌 unrar 解码器），且是近期的。

### 缓解与残余风险

| 风险 | 缓解 | 残余 |
| --- | --- | --- |
| 目录遍历 / symlink 绕过（上表 3 条） | **unrar 全程不碰文件系统**：`RAR_OM_EXTRACT` 打开 ＋ `RAR_TEST` 操作 ＋ `UCM_PROCESSDATA` 回调收字节，落盘位置由外壳决定 | 基本消除——这类绕过发生在 unrar 自己决定往哪写的时候，而它不再决定 |
| 解压炸弹 | 外壳的流式配额，超限回调返回 `-1` 立即中止 | 低 |
| 内存损坏 / RCE（上表 4 条） | **无法在进程内消除。** 只能靠：跟踪上游安全公告并及时升 vendor 版本；解包发生在任务线程而非 UI 线程；沙箱目录受 `cap_std` 约束 | **这是接受的风险，不是被消除的风险。** 进程级隔离未做，若日后判定必要，属独立切片 |

**长期义务**：unrar 是本项目唯一需要跟踪上游安全公告的依赖。vendor 版本必须可升，并纳入发版
前检查项。当前 vendor 基线：**UnRAR 7.23（2026-06-27）**。

## 容器判别契约

### 先开后嗅，绝不预检

**多种归档格式都允许整个归档不从文件首字节开始**，自解压形态就是这样：

- **zip**：归档定位靠从尾部倒着找 EOCD，不看首字节。
- **rar**：`archive.cpp:141-159` 的 `Archive::IsArchive` 在首字节没有签名时，
  **会往后扫最多 `MAXSFXSIZE`（4 MB，`rardefs.hpp:24`）寻找签名**。

> **「首字节不是某个 magic」推不出「不是那个格式」。**

判别顺序因此固定为：

```
1. 依次尝试每一种「当前已支持格式」的打开
     任一成功 → 用它，走既有路径
     全部失败 ↓
2. 嗅探容器 magic —— 用来「解释失败」，不是「拦截输入」
3. 命中已知但尚未支持的归档容器 → UnsupportedFormat(kind)
4. 命中已知的非归档文件         → NotAnArchive(kind)
5. 认不出，或看着像已支持格式却打不开 → Damaged（退回既有的 retry-hint）
```

**第 1 步是「每一种已支持格式」，不是「zip」。** 这不是措辞讲究——自解压 RAR 是个 `MZ` 开头的
`.exe`，若第 1 步写死成 zip，它在 T21-C 之后仍会被嗅探判成 `NotAnArchive`，
而 unrar 其实打得开。**支持集合是会增长的，判别必须跟着长。**

这个顺序有四个性质：

- **纯增量**：当前能导入的包，判别逻辑一次都不会执行到，零回归风险。
- **不依赖任何未验证的假设**：自解压 zip 若当前可用，预检会破坏它；若不可用，预检也不会让它
  变可用。两种情况下先开后嗅都正确。（它当前是否可导入，仍应作为回归守卫测一次。）
- **守住硬边界**：损坏的 zip 打不开又认不出，落到第 5 步，不会被误报成「格式不支持」。
- **支持集合增长时自动收敛**：某格式从「不支持」转为「支持」，它就从第 3 步移到第 1 步，
  嗅探表里对应的那一行自然失效，不需要额外维护档位。

**成本提示**：第 1 步的每次「尝试打开」都有真实开销。unrar 那 4 MB 的前缀扫描意味着
一个 4 MB 的随机文件会被完整扫一遍才判定不是 RAR。尝试顺序应把最常见的格式排在前面，
并且**不要为了「更准」去无限扩大尝试集合**。

### 语料只能在代码里合成——不能提交，也没有压缩器可用（2026-09-08 核实）

两条硬约束叠在一起，决定了语料的形态：

1. `policy/project-policy.json` 的 `forbiddenFiles.extensions` 含 **`.rar` / `.7z` / `.zip`**
   ——二进制语料**提交不进仓库**。
2. 开发机上**没有任何 RAR 压缩器**（WinRAR / `rar.exe` 都没有），7-Zip 也不能写 RAR。
   所以「先造一个再嵌进去」这条路也走不通。

结论：**RAR 语料由测试代码按格式规格现场合成**（store-only，不实现任何压缩算法
——这同时避开 UnRAR 许可对「重建 RAR 压缩算法」的禁止）。

**这不会造成假绿**：语料若拼错，unrar 直接打不开，测试硬失败。断言的形态是
「unrar 成功解出预期字节」，语料生成器的正确性被 unrar 自己反向校验，
不存在「生成器和读取器一起错」的可能——因为读取器不是我们写的。

真实 RAR 素材（solid、分卷、加密、RAR4 老世代）无法合成到位的部分，
留给维护者用真机素材验收，不在自动化网内谎称已覆盖。

### magic 常量必须由测试语料钉住

设计文档里列常量没有意义——**必须用真实样本文件在测试里钉住**，不能从规格书抄一遍就当成事实。
需要覆盖的容器（判别在首字节，`tar` 例外）：

| 容器 | 判别依据 | 归类 |
| --- | --- | --- |
| ZIP | `PK` 开头 | Damaged（能开就不会走到这里） |
| RAR4 / RAR5 | 首部 signature（两者不同） | UnsupportedFormat；**T21-C 后移入第 1 步** |
| 7z | 首部 signature | UnsupportedFormat；**T21-D 后移入第 1 步** |
| gzip / xz / bzip2 / zstd | 首部 signature | UnsupportedFormat |
| tar | **首字节无 magic**，`ustar` 在偏移 257 | UnsupportedFormat |
| PE 可执行文件 | `MZ` 开头 | NotAnArchive——**但只在所有 open 都失败之后**，见下 |
| MSI（OLE 复合文档） | 复合文档 signature | NotAnArchive |
| 其他 / 空文件 / 过短 | —— | Damaged |

两条实现陷阱：

- **`tar` 的判别要读到偏移 257**，与其他格式不同。**不要写成「统一读前 N 字节比对」**，
  否则 tar 会被静默归到 Damaged。
- **`MZ` 不等于「不是归档」。** 自解压 zip 与自解压 RAR 都是 `MZ` 开头。这一行之所以成立，
  完全依赖它位于**所有 open 尝试失败之后**；一旦有人把它提前成预检，两种自解压包立刻全废。

### unrar 只解 RAR（已核实）

查过 `D:\DEV\unrar` 的 7.23 源码：**目录里没有任何 zip / 7z / tar / cab 的处理文件**，
源码中对这些格式的提及全部是注释，或 `cmddata.hpp:11` 的 `DefaultStoreList`
——那是压缩时「哪些扩展名直接存储、不再尝试压缩」的设置，属压缩侧，与解压无关。

**因此 T21-D（7z）不能靠 unrar 顺带解决，必须单独做。**

反过来，RAR 的各代它全支持：`unpack15/20/30/50.cpp` ＋
`enum RARFORMAT { RARFMT_NONE, RARFMT14, RARFMT15, RARFMT50, RARFMT_FUTURE }`
（`archive.hpp:13`）覆盖 RAR 1.4 到 5.0。测试语料按 **RAR4 与 RAR5 各一份**即可覆盖两大分支。

## 失败档位映射

### 两档，不是一档

| 档位 | 命中 | 文案方向 |
| --- | --- | --- |
| `unsupported-archive-format` | rar / 7z / tar / gzip 家族 | 「这是 RAR 压缩包，HMM 目前只支持 ZIP」 |
| `not-an-archive` | PE / MSI 等非归档文件 | 「这看起来不是压缩包」 |
| `retry-hint`（既有） | 损坏、截断、认不出 | 「导入失败，请检查压缩包后重试」 |

分成两档而不是一档，是因为 **T22 拖拽批量导入的清单需要区分这两种情形**
（拖进来一个软件安装包 vs 拖进来一个我们读不了的压缩包），提示语和玩家的下一步动作都不同。

### 脱敏口径

`modImportTaskState.ts:40-41` 的既有规则：

> 失败原因只存语义，渲染时按当前界面语言取词；**绝不把后端事件内容拼进用户可见消息**。

本设计遵守且不放宽：后端只投影**语义码**（`rar` / `sevenz` / `tar` / `gzip` / `pe` / `msi` / …），
格式显示名由前端查表得到。后端的 `anyhow` 文本、libzip / unrar 的内部术语一律不出现在
用户可见文案里，也不进 `event.error`。

### 契约门禁

新增错误码需同步：
- 三语文案（CI 有错误码三语覆盖门禁，`cargo test` 全绿也照样会红）
- 若新增 Tauri 命令则需同步 `FRONTEND_BACKEND_CONTRACT.md`（本设计不新增命令）

## 安全外壳接口契约

### 端口签名要类型化，不要 downcast

`prepare_package` 当前返回 `anyhow::Result<PreparedModPackage>`。改为返回带语义的错误枚举，
并保留 `#[from] anyhow::Error` 兜底变体。

**明确不采用**「用 `anyhow` 上下文再 `downcast_ref` 取语义」这条路：一旦有人换个包装方式，
downcast 会静默失败并悄悄退回 `retry-hint`——那是这个仓库反复被坑过的那种假绿。类型化让语义
进入契约，测试可以穷尽断言。

迁移面：1 个生产实现 ＋ 8 个测试 fake，机械改动。

### 接口必须是推模式，不能是 `Read`

**这是接口形状上最要紧的一条，2026-09-08 落地 T21-B 时确定。**

rar 的 C API 是**回调式**的：解压器每产出一块就回调一次，回调返回中止信号即停。
若把外壳接口做成 pull（格式实现提供一个 `Read`），rar 适配器就只能**先把整个条目缓进内存**
再对外提供——那正是本设计在「已定的取舍二」里推翻过的形态。

zip 从 pull 转 push 是平凡的（一个 64 KiB 循环），反过来不是。
**接口形状必须迁就更受限的那一方。**

因此格式实现提供的是 `write_current_to(&mut self, sink: &mut dyn ArchiveChunkSink)`，
外壳提供的 sink 在每一块写入之前施加字节配额与取消检查。

### 外壳与格式的分工

**外壳负责**（对所有格式一视同仁）：

- 七条门禁：条目数、单文件大小、总解压量、`..` 逃逸、symlink/hardlink 等条目类型、
  大小写重名冲突、sandbox 约束
- 决定每个条目落盘到哪里，并实际执行写入
- 取消信号的传播

**格式实现只负责**：按顺序产出条目（名称、类型、可选的声明大小），以及按需产出条目的字节流。

### 接口的四条硬要求

这四条是从真实格式约束推出来的，不是偏好：

1. **声明大小允许「未知」。** rar 与 tar 都不保证在处理前拿到可信的总量。
2. **条目总数允许事先不可知。** zip 从中央目录预取 `archive.len()`（`:637`）；
   rar 要靠 `RARReadHeaderEx` 逐条顺序读。因此「条目数超限」的语义
   **从「预检拒绝」降级为「中途中止」**，这个降级是显式接受的，要在档位里说清楚。
3. **配额强制在字节流层，不在元数据层。** 见下节。
4. **条目类型枚举要能表达并拒绝** symlink、hardlink，以及（tar 家族恢复后的）字符/块设备、
   FIFO、绝对路径条目。zip 里 symlink 是权限位上的边缘特性，RAR5 里 symlink / hardlink 是
   一等条目类型。

### 这条要求不是理论洁癖——现存实现曾被它绕过（#367，已修）

规划本设计时曾把「配额挂字节流层」写成一条待实测的疑点。**2026-09-08 实测证实了最坏的情况，
并已由 `#367` / PR #368 修复。** 留档在此，因为它是这条接口要求的全部理由：

当时三道尺寸门禁读的全是 zip 头里的**声明**大小，而实际写盘的循环没有任何字节上限。
构造一个「声明 100 字节、实际 8 MiB」的包（只改 size 字段、不动 CRC，所以归档除声明外完全自洽）：

```
[probe 1] declared=100  produced=8388608  stopped_by=None
[probe 2] import SUCCEEDED; declared=100  landed_on_disk=8388608
```

`zip` 2.4.2 的读侧不截断且不报错；端到端**导入返回成功**，8 MiB 落进沙箱。
单文件 4 GiB 与总量 16 GiB 两道限额都能被一句谎话绕开。

**修法已落地，T21-B 直接沿用**：声明值预检保留（诚实的超大包在写第一个字节前就被拒，快速失败），
`copy_entry_with_byte_budget` 按累计**实际写入量**判定且**判定放在写之前**——会越线的那一块
直接中止，一个字节都不越界。

**因此 T21-B 在这一点上已经不是「要新建能力」，而是「把已有的正确语义推广到新格式」。**
新格式接进外壳时，这套配额必须复用，不得各写一份。

## rar 接入与许可义务

### 许可核对结果（已读原文）

UnRAR 许可第 2 条的关键句：

> UnRAR source code may be used in any software to handle RAR archives without limitations
> free of charge, but cannot be used to develop RAR (WinRAR) compatible archiver and to
> re-create RAR compression algorithm, which is proprietary.

| 条款 | 对本项目 |
| --- | --- |
| 禁止开发 RAR 兼容**压缩器**、禁止重建**压缩**算法 | **不触及**——我们只解压（另见「非目标」） |
| 允许作为其他软件的一部分分发 | 允许 |
| 条件：第 2 条**全文**须进 license；无 license 则进 documentation；**且**进源码注释 | 本仓库无 LICENSE 文件，走 documentation → `NOTICE.md`；源码注释由 vendor 目录自带 `license.txt` 承载 |
| 商业使用 | 无限制，未禁止收费 |
| 担保 | AS IS，作者不担责——须一并声明 |

### 交付义务清单

- vendor UnRAR 7.23 源码到独立目录，**原样保留 `license.txt`**（维护者明确要求：许可证不得丢失）
- 该目录标注「第三方原样引入，不得就地修改」，并排除出 review / policy 扫描
- `NOTICE.md` 补 UnRAR 许可**第 2 条全文**（自 "UnRAR source code" 起，逐字）与**第 4 条免责声明**
- **若日后为构建适配修改了 vendor 的源码，改动处必须在源码注释中带上第 2 条全文**
  ——这是许可明文条件，不是可选项

### FFI 面

官方 `dll.def` 共导出 12 个函数，本项目只需 6 个：

`RAROpenArchiveEx` / `RARCloseArchive` / `RARReadHeaderEx` / `RARProcessFileW` /
`RARSetCallback` / `RARGetDllVersion`

另需 `RAROpenArchiveDataEx`、`RARHeaderDataEx` 两个 struct 与一个回调。

#### 2026-09-08 落地 T21-C1 时核实到的偏差

本节原先几处说法在动手时被证伪，逐条改正并留下理由——它们都是「照规格书想当然」
会踩到的坑。

| 原文 | 实测 | 影响 |
| --- | --- | --- |
| 「Windows / MSVC 是当前唯一目标平台」 | **必须两个平台都编。** CI 跑 `ubuntu-latest`，发版才跑 `windows-latest`。限成 `cfg(windows)` 的话 CI 永远编不到、rar 的测试一条都不跑 | 双平台编译，CI 多花约 40 秒 |
| 「`cc` 编译 85 个 `.cpp`」 | **48 个。** 上游 `makefile` 的 `lib:` 目标是 `OBJECTS`＋`LIB_OBJ`；85 是目录里的总数，多编会把 CLI／SFX／恢复卷拖进来。Windows 另需 `isnt` / `motw` / `rs`（`UnRARDll.vcxproj` 独有），少了会链接期报 `MarkOfTheWeb::` 与 `WinNT` 未解析 | 文件清单**逐平台取自上游自己的配方** |
| 用 `RARProcessFile` | 改用 **`RARProcessFileW`**。`RAR_TEST` 下 `DestPath`/`DestName` 都被忽略，两者等价，W 变体不经 ANSI 代码页 | 攻击面更小 |
| 需要 `RARSetPassword` | **不声明。** 加密包不做密码交互（非目标）；不设密码时 unrar 自己返回 `ERAR_MISSING_PASSWORD`，正是要的明确档位 | 少一个永不调用的导出 |

另外两条不是偏差，是原文没提而必须写死的：

- **`dll.hpp` 顶部是 `#pragma pack(push, 1)`。** x64 上 `RARHeaderDataEx::CmtBuf`
  落在偏移 6188，不是 8 对齐位置；Rust 侧少写 `packed`，编译器会把它挪到 6192，
  此后每个字段错位且**无任何报错**。叠加 `wchar_t` 在 Windows 2 字节 / Linux 4 字节，
  手算布局是碰运气。因此 `hmm-unrar-sys/src/layout_probe.cpp` 让 C++ 自报
  `sizeof`／`offsetof`，Rust 测试逐字段核对。已反向验证：去掉任一 `packed`
  对应断言转红（`CmtBuf 6192≠6188`、`CmtBufW 72≠68`），而**功能性冒烟测试照旧通过**
  ——它对这类错位是瞎的。
- **MSVC 需要 `/utf-8`。** 中文 Windows 上它按 GBK 读源文件，UTF-8 注释里的三字节
  序列会把行尾换行吃掉，把下一行 `#include` 吞进注释——症状是「结构体未声明」，
  与编码毫无相似之处。

### C2 实测出来的六条约束（全部指向 C3）

语料生成器一落地，用 unrar 自己去读合成的归档，立刻撞出六件「不实测就一定写错」的事。
逐条记在这里，因为它们全都是 C3 的实现约束，而不是可选优化。

| # | 事实 | 对 C3 的约束 |
| --- | --- | --- |
| 1 | **unrar 不能并发调用。** 8 条往返用例并行跑只过 2 条、其余报 `ERAR_UNKNOWN`；`--test-threads=1` 立刻变 6 条。机制对得上源码：`global.hpp:10` 是 `EXTVAR ErrorHandler ErrHandler;`，**整个进程共用一个错误处理器** | 整条「打开→读头→处理→关闭」序列必须持进程级锁。已做成 `Archive` 这个 RAII 值，**拿不到它就调不到 unrar**——漏取锁从「要记得」变成「不可能」 |
| 2 | **条目名分隔符随平台不同**：Windows 报 `\`、Linux 报 `/` | 适配器必须归一成 `/`，否则**同一份素材在两个平台上解出不同的目录结构**：`dir\file.txt` 在 Windows 上是两层，在 Linux 上是一个名字带反斜杠的单文件，后面的内容根识别与安装全跟着错。**不是包含性问题**——落盘全程在 `cap_std` 能力内，不归一也逃不出沙箱，只是名字古怪。这条测试**只在 Linux 上才承重**（Windows 侧 `Path` 本来就认反斜杠），而 CI 恰好跑 Linux |
| 3 | **回调返回 `-1` 中止后，`RARProcessFileW` 返回 `ERAR_UNKNOWN`**：`RARX_USERBREAK` 不在 `dll.cpp:497` 的映射表里，落到 `default` | 适配器**必须自己记住中止原因**。否则外壳那些精确文案（「超出单文件上限」「已取消」）会被一句泛化错误盖掉，共享负测直接挂 |
| 4 | **「解压大小未知」报的是哨兵 `0x7fffffff_7fffffff`**（`rartypes.hpp:30` 的 `INT64NDF`），不是 0 | 必须翻译成 `declared_size: None`。照原样用的话，声明值预检会拿 9.2e18 去比 4 GiB 上限，把一个好包直接拒掉，字节流配额根本轮不到跑 |
| 5 | **条目名超过 1023 宽字符会被静默截断**（`dll.cpp:265` 的 `wcsncpyz`，除非调用方给 `FileNameEx` 缓冲区） | 名字填满缓冲区时**拒绝该条目**。截断后的路径是错的数据，可能指向别处或与其他条目撞名。恰好 1023 字符的合法名与被截断的长名无法区分，所以一律判可疑——宁可错拒一个病态长名 |
| 6 | **store 方式结构上无法「声明得比实际小」**：`extract.cpp:902` 的 `UnstoreFile` 以 `UnpSize` 为界 | 共享负测的 `LyingDeclaredSize` 一例，rar 侧按**「声明未知」**（`FHFL_UNPUNKNOWN`）来造。断言与期望文本一字不改——该用例的意图本就是「配额不能依赖声明值」，「未知」对这条意图的检验不比「说谎」弱 |

### unrar 只能按路径打开归档，而本仓库的解包链路是 reader 形态

`dll.cpp:72` 是 `Data->Arc.Open(ArcName, …)`：**DLL API 只接受路径，没有任何
句柄／回调式的读入口**。而 `ModImportPackagePreparer` 的两个入口里，
`prepare_package_from_reader` 拿到的是 `&mut dyn Read + Seek`，
其存在理由恰恰是「让适配器保住 no-follow 能力链」。

这条冲突必须在 T21-C3 显式解决，不能糊过去。**已定的方向：把 reader 落盘到一个
由外壳创建、用完即删的暂存目录，再把那个路径交给 unrar。** 理由：

- unrar 从头到尾只读**我们自己刚创建的文件**，玩家给的路径不进它的视野
  ——symlink／junction 跟随与 TOCTOU 一并消失，比传真实路径更强，
  而不是更弱
- 两个入口共用一条实现，不产生「只有一边被测到」的分支
- 暂存放在**包沙箱之外**：包沙箱的内容会被原样提交成 Mod 版本，
  把原始压缩包丢进去会污染包体

代价是一次完整拷贝。对本设计的动机素材（55 MB）可忽略；即使 1 GB 级，
拷贝耗时相对解压也是小头。**若日后判定不可接受，属独立优化，不是安全折衷。**

### 使用模式（不可协商）

```
RAROpenArchiveEx(RAR_OM_EXTRACT)
  → RARSetCallback(收字节的回调)
  → 循环 RARReadHeaderEx + RARProcessFile(RAR_TEST)
```

用 `RAR_TEST` 而不是 `RAR_EXTRACT`：前者不产生文件系统写入，字节全部经 `UCM_PROCESSDATA`
回调交给外壳。**任何让 unrar 自己往目录里写的用法都不允许**，那会使外壳对 rar 完全失效。

## 安全测试矩阵

### 判别层（纯字节，无需 fixture、不碰文件系统）

| 用例 | 期望（T21-A 时） | T21-C 之后 |
| --- | --- | --- |
| 真实 zip | 正常导入（控制组） | 不变 |
| 截断的 zip | `retry-hint`——**不得**被新档位吃掉 | 不变 |
| **`MZ` 前缀 ＋ 追加真 zip（自解压形态）** | 与不加前缀时**行为一致**（回归守卫） | 不变 |
| RAR4 / RAR5 | `unsupported-archive-format`，格式码正确 | **正常导入** |
| **自解压 RAR（`MZ` 开头，签名在 4 MB 内）** | `not-an-archive` | **正常导入**——见下 |
| 7z | `unsupported-archive-format` | T21-D 后正常导入 |
| gzip / xz / bzip2 / zstd | `unsupported-archive-format`，格式码各自正确 | 不变 |
| tar（`ustar` 在偏移 257） | `unsupported-archive-format`，**不得**落到 Damaged | 不变（本轮延后） |
| PE（非自解压）/ MSI | `not-an-archive` | 不变 |
| 空文件 / 1 字节 / 全零 | `retry-hint`，不 panic | 不变 |

**自解压 RAR 那一行是这张表里唯一「期望值随切片改变」的用例，因此它是判别顺序是否真的
按「支持集合」推进的活体断言。** 若 T21-C 之后它仍报 `not-an-archive`，说明第 1 步被写死成了
固定格式列表——这正是本设计要防的那个实现陷阱。**T21-C 必须包含把这一行从
`not-an-archive` 翻转为「正常导入」的测试改动**，翻不过来就是没做对。

### 各格式共用的负测

**同一组负测，每种格式都要过，且不得为任何格式重写一份：**
解压炸弹、路径逃逸、symlink 条目、条目数超限。

rar 另加：hardlink 条目、分卷、加密包各自落到明确档位。RAR 世代覆盖按 RAR4 ＋ RAR5 两份。

> **「要重写就说明外壳没抽干净」——这是 T21-B 的验收判据本身，不是附加要求。**

### 反向验证

先 commit 再施加突变，`cargo test` 用 `--no-fail-fast`（否则第一个 test binary 失败后
跨文件的转红会被静默漏报）。

| 突变 | 必须转红的用例 | 证明什么 |
| --- | --- | --- |
| 删掉嗅探分支 | `.rar` 退回 `retry-hint` | 新档位由判别驱动，而非恒定命中 |
| 嗅探恒返回 `Rar` | **截断的 zip** 用例 | 「不能把损坏说成不支持」真的被测到 |
| 外壳的字节配额改成不生效 | 解压炸弹用例（**每种格式各一条**） | 配额确实施加在外壳层，不是各格式各写一份 |

**断言要钉住意图**：真实 zip 的用例要断言「产出的 prepared package 与改动前**等价**」，
而不只是「没报错」。

## 分阶段实施计划

### T21-A：失败原因通道与两个档位

- 端口错误类型化（`#[from] anyhow::Error` 兜底），失败事件投影语义码
- 容器判别（先开后嗅），两个新档位
- 前端失败分支改为读 `event.error` 映射档位，`retry-hint` 作为兜底
- 三语文案

**提交边界**：后端档位与错误码一个提交；前端档位与三语文案一个提交。

**完成定义**：`.rar` 得到可操作提示；截断的 zip 仍报 `retry-hint`；自解压形态无行为变化；
上表两条反向验证通过。

### T21-B：安全外壳与格式解耦

- 七条门禁上移到与格式无关的一层，格式实现只提供条目迭代
- 接口满足前述四条硬要求
- 字节流配额已由 #367 落地，此处**复用**而非重建；新格式不得各写一份

#### 一个规划矛盾及其解法（2026-09-08）

原完成定义写的是「rar 与 zip 共用同一组负测」——**那让 B 的完成定义依赖 C**。
tar 当验收载体时不存在这个问题（tar 便宜，与 B 同一个 PR 就落地），换成 rar 之后 B 就
没法自证了。三条路里选了第三条：

| 路线 | 否决理由 |
| --- | --- |
| B＋C 一个 PR | 重构 ＋ 150 个 vendor 文件 ＋ FFI ＋ 测试挤在一起，diff 无法连贯 review |
| B 先合、C 再验 | B 看着像做完了，等 C 发现接口形状不对时重构已经进 main |
| **B 带一个「有形状的假格式」** | **选定** |

**这与本文档先前否掉的「假格式」不是一回事**：否掉的是「故意不做任何检查」的假格式，
它只能证明外壳**会调用**检查。这里用的是**刻意做成与 zip 形状相反**的测试用格式，
把 rar 将会带来的每一个形状差异提前摆出来：

- 条目总数**事先不可知**（`declared_entry_count()` 返回 `None`）
- 声明大小**可以未知**，也**可以说谎**
- 能产出 symlink / hardlink / device / fifo 条目
- 字节是**推**给 sink 的，不是被 `Read` 拉走的

**完成定义（自足）**：

1. 整组负测跑在这个与 zip 形状相反的格式上，**断言与期望文本一条都不为它重写**
2. 既有的 zip 侧测试全部照原样通过——重构不改变行为
3. **C 落地时外壳接口零改动**

**第 3 条是硬约束**：若 C 需要改动外壳接口，就是 B 做错了的信号，
**必须在 C 的 PR 里显式说明改了什么、B 为什么没预见到**——不允许悄悄改完当没事。

### T21-C：rar

**C1（已实现）**：`hmm-unrar-sys` —— vendor UnRAR 7.23、双平台 `build.rs`、
6 个 FFI 声明、布局探针与断言、`NOTICE.md` 许可义务、policy 排除、
`.gitattributes` 保住上游字节。**不接导入链路**，`hmm-infra` 尚未依赖它。

**C2（已实现）**：store-only RAR5 语料生成器 ＋ unrar 往返验证 ＋ `Archive` RAII
（把「必须持锁」做成类型约束）。

**C3（已实现）**：`RarArchiveSource` 接入外壳、判别第 1 步扩容、暂存目录、
加密 / 分卷两档 ＋ 三语文案、文件对话框过滤器。

C 整体的完成定义与兑现情况：

| 完成定义 | 状态 |
| --- | --- |
| vendor UnRAR 7.23 ＋ `build.rs` ＋ 6 个 FFI 声明 | ✅ C1 |
| 按「使用模式」（`RAR_OM_EXTRACT` ＋ `RAR_TEST` ＋ 回调）接入外壳 | ✅ C3 |
| 把 rar 加进判别第 1 步，自解压 RAR 期望从 `not-an-archive` 翻成「正常导入」 | ✅ C3，用例 `a_self_extracting_rar_now_imports_instead_of_being_called_not_an_archive` |
| `NOTICE.md` 许可义务落地 | ✅ C1 |
| **外壳接口零改动**（T21-B 的硬约束） | ✅ 三个 trait 与七条门禁一行未动 |
| 共享负测整组跑在 rar 上、一条不重写 | ✅ C3 |
| 加密包落明确档位 | ✅ `mod_import_archive_encrypted` |
| 分卷包落明确档位 | ✅ `mod_import_archive_multi_volume`，且在 open 阶段就报得出来 |
| RAR4 老世代 | ⬜ **未覆盖**：语料生成器只写 RAR5。unrar 本身支持 RAR 1.4–5.0（`archive.hpp:13`），但我们造不出 RAR4 语料 |
| solid 包 | ⬜ **未覆盖**：store-only 语料构不成真实的固实链，`MHFL_SOLID` 标志位测得到、实际行为测不到 |

**⬜ 两项留给维护者用真机素材验收，不在自动化网内谎称已覆盖。**
理由见「语料只能在代码里合成」一节：开发机上没有任何 RAR 压缩器。

### T21-D：7z

纯 Rust，无 C 依赖。solid block 整块解压，与 rar 同类——B 的接口若做对了，这里是复用而非重来。
加密包（7z 支持头加密）必须落到明确档位，不能卡死或提示不清。

### T21-E：`.tar` 家族

`out_of_scope`（本轮）。分析保留在 [任务总纲](../TODO.md) T21-E，恢复时直接用。
要点：`.tar.gz` 的压缩层在容器**外面**，配额必须挂解压器输出流；
每个编解码依赖单独过许可，不得因为「都是 tar 家族」批量放行。

## 停止条件与开放问题

- ~~vendor 目录的 policy 排除方式未定~~ **已落地（T21-C1）**：
  `policy/project-policy.json` 的 `checkScopes.{preCommit,verify}.excludePathPatterns`
  与 `fileSize.excludePathPatterns` 各加一条 vendor 路径（是两处，不是一处）。
  **如实说明：这三条当前不改变任何检查结果**——vendor 全是 `.cpp`/`.hpp`，不在
  `fileSize.extensions` 映射里，也不含违禁扩展名，secret scan 亦通过。加它是为了
  日后有人调门禁或往映射里加 `.cpp` 时，第三方树不会被当成我们的代码来评判。
  另外 `.gitattributes` 给 vendor 树加了 `-text`：仓库默认 `eol=lf` 会改写上游的 CRLF，
  那样 vendor 就不再是「原样」的了，`license.txt` 更不该被动一个字节。
- **进程级隔离未做**：内存安全类残余风险靠「跟踪上游 ＋ 及时升版」承担。
  若日后判定不可接受，属独立切片，不在本设计内。
- **`unrar` 升级流程未定**：谁盯上游公告、多久一次、怎么验证升级后行为不变。
- **本仓库无 LICENSE 文件**：与本设计正交（UnRAR 许可明文允许退到 documentation），
  但会长期影响贡献者版权归属，应单独处理。

## 固定参考

- 缺陷立案：#348
- 任务状态与切片清单：[任务总纲](../TODO.md) T21
- 下游依赖此设计的功能：T22 拖拽批量导入（#366）——其待导入清单的档位直接复用本设计的
  `unsupported-archive-format` / `not-an-archive` 两档
- 相关既有设计：[第三方 Mod 管理器批量迁移设计](EXTERNAL_MOD_MANAGER_BATCH_IMPORT_DESIGN.md)
