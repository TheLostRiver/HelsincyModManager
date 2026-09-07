// 让 C++ 侧自己报出结构体布局，供 Rust 单测逐字段核对。
//
// **这不是洁癖。** `dll.hpp` 顶部是 `#pragma pack(push, 1)`，所以 x64 上
// `RARHeaderDataEx::CmtBuf` 这个指针落在偏移 6188——不是 8 对齐的位置。
// Rust 侧若少写了 `packed`，编译器会把它挪到 6192，此后每一个字段都错位，
// 而且**不会有任何报错**：只会读到垃圾长度、垃圾指针，然后在解压真实归档时
// 以难以归因的方式崩掉或读错。
//
// 更麻烦的是 `wchar_t`：Windows 上 2 字节、Linux 上 4 字节，所以同一个结构体
// 在两个平台上大小根本不同。手算两遍再抄进 Rust 是纯粹的碰运气。
//
// 这个文件是**本项目自己写的**，不在 vendor 目录内，因此不触及「vendor 原样保留、
// 不得就地修改」的约束。
// 走 `rar.hpp` 而不是直接 include `dll.hpp`：后者用到的 `HANDLE` / `LPARAM` /
// `CALLBACK` / `UINT` 只在 `_UNIX` 分支里自己定义，Windows 上要靠 `os.hpp` 先拉进
// `windows.h`。更重要的是——这样探针看到的预处理环境与真正参与编译的 48 个
// 翻译单元完全一致，量出来的才是 unrar 自己用的那个布局。
#include "rar.hpp"

#include <stddef.h>

extern "C" {

size_t hmm_unrar_sizeof_header_data_ex(void) { return sizeof(struct RARHeaderDataEx); }
size_t hmm_unrar_sizeof_open_archive_data_ex(void) { return sizeof(struct RAROpenArchiveDataEx); }
size_t hmm_unrar_sizeof_wchar(void) { return sizeof(wchar_t); }

size_t hmm_unrar_offsetof_header_file_name_w(void) {
  return offsetof(struct RARHeaderDataEx, FileNameW);
}
size_t hmm_unrar_offsetof_header_flags(void) { return offsetof(struct RARHeaderDataEx, Flags); }
size_t hmm_unrar_offsetof_header_unp_size(void) {
  return offsetof(struct RARHeaderDataEx, UnpSize);
}
size_t hmm_unrar_offsetof_header_unp_size_high(void) {
  return offsetof(struct RARHeaderDataEx, UnpSizeHigh);
}
// 这一条是全部理由所在：packed 与否，它的值差 4。
size_t hmm_unrar_offsetof_header_cmt_buf(void) { return offsetof(struct RARHeaderDataEx, CmtBuf); }
size_t hmm_unrar_offsetof_header_redir_type(void) {
  return offsetof(struct RARHeaderDataEx, RedirType);
}
size_t hmm_unrar_offsetof_header_file_attr(void) {
  return offsetof(struct RARHeaderDataEx, FileAttr);
}

size_t hmm_unrar_offsetof_open_open_mode(void) {
  return offsetof(struct RAROpenArchiveDataEx, OpenMode);
}
size_t hmm_unrar_offsetof_open_open_result(void) {
  return offsetof(struct RAROpenArchiveDataEx, OpenResult);
}
size_t hmm_unrar_offsetof_open_flags(void) {
  return offsetof(struct RAROpenArchiveDataEx, Flags);
}
size_t hmm_unrar_offsetof_open_callback(void) {
  return offsetof(struct RAROpenArchiveDataEx, Callback);
}
size_t hmm_unrar_offsetof_open_user_data(void) {
  return offsetof(struct RAROpenArchiveDataEx, UserData);
}
size_t hmm_unrar_offsetof_open_op_flags(void) {
  return offsetof(struct RAROpenArchiveDataEx, OpFlags);
}
// `RAROpenArchiveDataEx` 里 packed 与否唯一分道的位置：`OpFlags`（4 字节，偏移 64）
// 之后跟一个指针，packed 下落在 68，对齐下被推到 72。它前面的字段全都天然对齐，
// 所以少了这一条，去掉 packed 就只能靠 sizeof 兜住——而 sizeof 是可能被抵消的。
size_t hmm_unrar_offsetof_open_cmt_buf_w(void) {
  return offsetof(struct RAROpenArchiveDataEx, CmtBufW);
}
size_t hmm_unrar_offsetof_open_mark_of_the_web(void) {
  return offsetof(struct RAROpenArchiveDataEx, MarkOfTheWeb);
}
}
