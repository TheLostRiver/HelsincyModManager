# hmm-unrar-sys

vendor 进来的 **UnRAR 7.23**（2026-06-27）静态库 ＋ 最小 FFI 面。
`.rar` 导入的解压能力由它提供；**安全策略一条都不在这里**，全部在
`hmm-infra` 的 `archive_extraction` 外壳里，对所有格式一视同仁。

## vendor/ 是第三方原样副本，不得就地修改

`vendor/unrar/` 与上游发布逐字节相同（`.gitattributes` 关掉了行尾归一化）。
`license.txt` 随源码保留，许可义务落地见仓库根的 [NOTICE.md](../../../NOTICE.md)。

**改这棵树之前先想清楚**：UnRAR 许可要求「修改后的源码分发时，改动处须在源码注释里
带上许可第 2 条全文」。为构建适配而改，代价是这条义务；能在 `build.rs` 里解决的
一律在 `build.rs` 里解决。

升级上游版本的做法是**整树替换**，然后跑 `cargo test -p hmm-unrar-sys`：
布局断言会告诉你 ABI 有没有变。

## 为什么两个平台都编

CI 跑在 ubuntu-latest，发版跑 windows-latest。把这个 crate 限成 `cfg(windows)`
的话，CI 永远编不到它、rar 的测试一条也不会跑——「CI 全绿但根本没执行这部分」
是本仓库反复吃过亏的假绿形态。所以宁可让 CI 多花 40 秒编 48 个 C++ 文件。

两个平台的文件清单与宏定义分别取自上游自己的配方（`makefile` 的 `lib:` 目标、
`UnRARDll.vcxproj`），不是猜的；差异与理由写在 `build.rs` 的注释里。

## 布局断言是这个 crate 的核心资产

`dll.hpp` 顶部是 `#pragma pack(push, 1)`，而 `wchar_t` 在 Windows 是 2 字节、
Linux 是 4 字节。两条叠在一起，手算结构体布局是纯碰运气，算错了**不会有任何报错**
——只会在解压真实归档时以难以归因的方式发作。

所以 `src/layout_probe.cpp` 让 C++ 自己报 `sizeof` / `offsetof`，`src/lib.rs`
底部的测试逐字段核对。已反向验证：去掉任一个 `packed`，对应那条断言转红；
而功能性冒烟测试（打开不存在的归档）**照旧通过**——它对这类错位是瞎的。
