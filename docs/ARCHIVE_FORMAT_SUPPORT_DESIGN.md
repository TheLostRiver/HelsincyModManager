# 导入压缩格式支持设计

> 状态（2026-09-08）：T21 全链 `design-complete`，尚无实现。切片顺序
> **A 失败档位 → B 安全外壳与格式解耦 → C rar → D 7z**；`.tar` 家族（T21-E）本轮延后。
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
| 成本 | 低 | `build.rs`（`cc` 编译 85 个 `.cpp`）＋ 6 个 `extern "C"` ＋ 2 个 struct ＋ 1 个回调 |
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

zip 允许整个归档**追加在别的文件之后**（自解压 exe 即是此形态），归档定位靠从尾部倒着找
EOCD，而不是靠首字节。因此：

> **「首字节不是 `PK` 」推不出「不是 zip」。**

判别顺序因此固定为：

```
1. 照旧尝试按 zip 打开
     成功  → 走原路径，一个字节不改
     失败  ↓
2. 嗅探容器 magic —— 用来「解释失败」，不是「拦截输入」
3. 命中已知的非 zip 容器 → UnsupportedFormat(kind)
4. 命中已知的非归档文件 → NotAnArchive(kind)
5. 首字节是 PK，或认不出 → Damaged（退回既有的 retry-hint）
```

这个顺序有三个性质：

- **纯增量**：当前能导入的包，判别逻辑一次都不会执行到，零回归风险。
- **不需要先证明自解压 zip 当前可用**：若可用，预检会破坏它；若不可用，预检也不会让它变可用。
  两种情况下先开后嗅都是正确选择，**因此这条设计不依赖任何未验证的假设**。
  （自解压 zip 当前是否可导入，仍应作为回归守卫测一次，见「安全测试矩阵」。）
- **守住硬边界**：损坏的 zip 首字节是 `PK`，落到第 5 步，不会被误报成「格式不支持」。

### magic 常量必须由测试语料钉住

设计文档里列常量没有意义——**必须用真实样本文件在测试里钉住**，不能从规格书抄一遍就当成事实。
需要覆盖的容器（判别在首字节，`tar` 例外）：

| 容器 | 判别依据 | 归类 |
| --- | --- | --- |
| ZIP | `PK` 开头 | Damaged（能开就不会走到这里） |
| RAR4 / RAR5 | 首部 signature（两者不同） | UnsupportedFormat（T21-C 后转为支持） |
| 7z | 首部 signature | UnsupportedFormat（T21-D 后转为支持） |
| gzip / xz / bzip2 / zstd | 首部 signature | UnsupportedFormat |
| tar | **首字节无 magic**，`ustar` 在偏移 257 | UnsupportedFormat |
| PE 可执行文件 | `MZ` 开头 | NotAnArchive |
| MSI（OLE 复合文档） | 复合文档 signature | NotAnArchive |
| 其他 / 空文件 / 过短 | —— | Damaged |

`tar` 的判别需要读到偏移 257，这与其他格式不同，**实现时不要写成「统一读前 N 字节比对」**，
否则 tar 会被静默归到 Damaged。

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

### 一条待实测的现存疑点

**读代码发现，未实测，不作为结论：**

当前上限判据用的是 zip 头里的**声明**大小（`:646` 的 `entry.size()`、`:658` 的累加），
而实际写盘走 `copy_with_cancellation`（`:718`）——**该函数读到 EOF 为止，没有字节上限**。
即「校验的是声明值，写入的是流」。`zip` 2.4.2 的读侧是否已按声明的解压大小截断，需要实测。

**核实方法**：构造一个声明大小远小于实际解压量的 zip，量沙箱里落盘多少字节。

**这个答案决定 T21-B 的性质**：若确实没截断，B 就不只是重构，而是在修一个现存的口子，
优先级与写法都不同。**探针应在 T21-A 期间完成，不阻塞 A。**

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

`RAROpenArchiveEx` / `RARCloseArchive` / `RARReadHeaderEx` / `RARProcessFile` /
`RARSetCallback` / `RARSetPassword`

另需 `RAROpenArchiveDataEx`、`RARHeaderDataEx` 两个 struct 与一个回调。
`build.rs` 用 `cc` 编译，定义 `RARDLL`；Windows / MSVC 是当前唯一目标平台。

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

| 用例 | 期望 |
| --- | --- |
| 真实 zip | 正常导入（控制组） |
| 截断的 zip | `retry-hint`——**不得**被新档位吃掉 |
| **`MZ` 前缀 ＋ 追加真 zip（自解压形态）** | 与不加前缀时**行为一致**（回归守卫） |
| RAR4 / RAR5 / 7z / gzip / xz / bzip2 / zstd | `unsupported-archive-format`，格式码各自正确 |
| tar（`ustar` 在偏移 257） | `unsupported-archive-format`，**不得**落到 Damaged |
| PE / MSI | `not-an-archive` |
| 空文件 / 1 字节 / 全零 | `retry-hint`，不 panic |

### 各格式共用的负测

**同一组负测，每种格式都要过，且不得为任何格式重写一份：**
解压炸弹、路径逃逸、symlink 条目、条目数超限。

rar 另加：hardlink 条目、分卷、加密包各自落到明确档位。

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
- 待实测疑点若成立，一并修掉

**验收载体是 rar**（原计划是裸 `.tar`，随 tar 家族延后而改）。rar 同样破掉「条目总数可预取」
与「条目类型只有文件/目录」两个假设，只是成本更高——**因此更没有理由跳过 B 直接写 C**。

**完成定义**：rar 与 zip 共用同一组负测，一条都不为 rar 单独重写。

### T21-C：rar

- vendor UnRAR 7.23 ＋ `build.rs` ＋ 6 个 FFI 声明
- 按「使用模式」接入外壳
- `NOTICE.md` 许可义务落地
- RAR5、solid 包、分卷、加密包各自落到明确档位

### T21-D：7z

纯 Rust，无 C 依赖。solid block 整块解压，与 rar 同类——B 的接口若做对了，这里是复用而非重来。
加密包（7z 支持头加密）必须落到明确档位，不能卡死或提示不清。

### T21-E：`.tar` 家族

`out_of_scope`（本轮）。分析保留在 [任务总纲](../TODO.md) T21-E，恢复时直接用。
要点：`.tar.gz` 的压缩层在容器**外面**，配额必须挂解压器输出流；
每个编解码依赖单独过许可，不得因为「都是 tar 家族」批量放行。

## 停止条件与开放问题

- **vendor 目录的 policy 排除方式未定**：150 个第三方文件不应进 review 面与 lint。
  `policy/project-policy.json` 目前 `excludePathPatterns` 为空，需要在 T21-C 前定。
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
