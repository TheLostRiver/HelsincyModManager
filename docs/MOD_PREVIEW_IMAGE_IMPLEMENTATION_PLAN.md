# Mod 预览图安全处理实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**目标：** 按 [Mod 预览图安全处理设计](MOD_PREVIEW_IMAGE_PIPELINE_DESIGN.md) 落地一条安全、可测试、可降级的预览图处理流水线，让前端只消费后端生成的受控缩略图或 fallback 状态。

**架构：** 预览图处理属于 Mod 导入分析 prepare 阶段，不进入游戏目录写锁，也不由前端读取任意本地路径。`hmm-core` 定义策略和值对象，`hmm-ports` 定义扫描、处理、缓存接口，`hmm-infra` 处理真实 I/O 和图片解码，`hmm-app` 编排导入任务，Tauri command 只暴露受控 DTO，前端只消费与 DTO 对齐的本地 `PreviewImage` union。

**技术栈：** Rust workspace、Tauri 2、React + TypeScript、`image` crate 或经验证的等价图片处理库、临时目录测试、人工构造的最小图片样本。

---

## 适用范围

本文是实现计划，不替代长期设计文档。长期约束以 [Mod 预览图安全处理设计](MOD_PREVIEW_IMAGE_PIPELINE_DESIGN.md)、[前后端通信契约设计](FRONTEND_BACKEND_CONTRACT.md)、[安全策略](../SECURITY.md) 和 [测试指南](TESTING.md) 为准。

本计划覆盖：

- 后端预览图策略、结果模型和拒绝原因。
- 安全解压后的候选图扫描接口。
- 图片文件大小、magic bytes、header 尺寸、像素数、解码和转码限制。
- 缩略图缓存的受控引用。
- Mod 导入结果中的 `previewImage` DTO。
- 前端 Mod 卡片展示缩略图、加载失败回退和占位图。
- 单元测试、集成测试和前端样式回归测试。

本计划不覆盖：

- 完整 Mod 安装、卸载、回滚流程。
- 用户手动裁剪、手动选择候选图或详情页大图。
- 真实第三方 Mod 包样本。
- 前端任意文件选择并直接作为卡片图展示。

## 安全原则

- 第三方 Mod 包中的原始图片永远是不可信输入。
- 原始导入包只读，缩略图是可删除、可重建的派生缓存。
- 前端不能提交真实缓存路径、压缩包内部路径或本地图片路径要求后端读取。
- 后端返回给前端的图片引用必须是受控资源 URL 或 opaque `thumbnailRef` 解析结果。
- 图片处理失败返回 fallback，不阻断 Mod 导入主流程。
- 如果包级安全校验发现路径穿越、压缩炸弹或其他高风险问题，由导入流水线阻断，预览图模块不单独覆盖包安全判定。
- 日志记录结构化字段和错误码，不记录完整本地路径、第三方图片内容、Windows/Linux 用户名、Steam ID、token、cookie 或真实 Mod 内容。

## 文件结构

后续实现建议按下列边界拆分，避免把图片 I/O、领域策略、Tauri DTO 和前端展示塞进同一文件。

### Rust 领域层

- 创建 `src-tauri/crates/hmm-core/src/preview_image.rs`
  - 定义 `PreviewImagePolicy`、`PreviewImageStatus`、`PreviewImageRejectionReason`、`PreviewImageOutputFormat`。
  - 提供保守默认策略和策略校验。
- 修改 `src-tauri/crates/hmm-core/src/lib.rs`
  - re-export 预览图领域类型。
- 测试 `src-tauri/crates/hmm-core/src/preview_image.rs`
  - 覆盖默认值、策略校验和序列化稳定性。

### Rust Ports

- 创建 `src-tauri/crates/hmm-ports/src/preview_image.rs`
  - 定义 `PackagePreviewScanner`、`PreviewImageProcessor`、`ThumbnailStore`。
  - 定义 `PreviewImageCandidate`、`PreviewImageSourceRef`、`ThumbnailRef`、`ProcessedPreviewImage`。
- 修改 `src-tauri/crates/hmm-ports/src/lib.rs`
  - re-export ports 和 DTO 风格的内部类型。

### Rust Infra

- 创建 `src-tauri/crates/hmm-infra/src/preview_image/mod.rs`
  - 组合真实 scanner、processor、thumbnail store。
- 创建 `src-tauri/crates/hmm-infra/src/preview_image/magic_bytes.rs`
  - 只负责识别 PNG、JPEG、WebP magic bytes。
- 创建 `src-tauri/crates/hmm-infra/src/preview_image/processor.rs`
  - 执行文件大小检查、magic bytes、header 尺寸读取、像素数限制、解码、缩放、编码。
- 创建 `src-tauri/crates/hmm-infra/src/preview_image/thumbnail_store.rs`
  - 原子写入应用数据目录下的缩略图缓存，返回 opaque `ThumbnailRef`。
- 修改 `src-tauri/crates/hmm-infra/src/lib.rs`
  - re-export infra 实现。
- 修改 `src-tauri/crates/hmm-infra/Cargo.toml`
  - 引入图片处理依赖。优先使用 `image` crate 的最小必要 features；输出格式策略支持 WebP/JPEG。如果 WebP 编码链在目标平台不稳定，MVP 可以先用 JPEG，契约仍保留 `preferred_output_format`。

### Rust App

- 创建 `src-tauri/crates/hmm-app/src/preview_image.rs`
  - 定义 `PreviewImageService`，编排候选扫描、策略应用、处理、缓存和 fallback。
  - 不直接依赖具体文件系统或图片库。
- 修改 `src-tauri/crates/hmm-app/src/lib.rs`
  - re-export service。
- 后续接入 Mod 导入流水线时修改对应 import service
  - 将 `PreviewImageService` 放在安全解压和包结构分析之后、元数据写入之前。
  - 当前已落地 `hmm-app::ModImportAnalysisService` 作为导入分析骨架：输入后端生成的 `task_id`、`package_id` 和安全 sandbox 路径，输出带 `ImportPreviewImage` 的分析结果。真实 `start_import_mod_task`、持久化和 library/detail command 仍属于后续导入流水线任务。

### Tauri DTO 与 Command

- 扩展 `src-tauri/src/dto.rs`
  - 定义 `PreviewImageDto`、`PreviewImageFallbackReasonDto`，保持 `serde(rename_all = "camelCase")`。
- 后续接入 Mod 导入 command 时修改对应 command 文件
  - `start_import_mod_task` 返回任务 id，最终结果通过 `get_mod_library` / `get_mod_detail` 暴露 `previewImage`。
  - 不新增 `read_image_path`、`load_preview_from_path` 这类宽泛 command。
- 如果使用 Tauri asset protocol 或自定义资源解析
  - 只允许根据 `ThumbnailRef` 解析应用缓存内的缩略图。
  - 不允许前端直接访问任意本地路径。

### 前端

- 创建 `src/features/mods/modPreviewImageTypes.ts`
  - 定义 `PreviewImage` union，与后端 DTO 对齐。
- 修改 `src/features/mods/modsLibraryData.ts`
  - 给 `ModLibraryItem` 增加 `previewImage?: PreviewImage`。
  - mock 数据默认使用 fallback，避免引入真实图片资源。
- 修改 `src/features/mods/ModPosterCard.tsx`
  - 有 thumbnail 时渲染 `<img className="mod-card__poster-img" loading="lazy" decoding="async" alt="" />`。
  - `onError` 后切回当前剪影占位。
  - tech 视图继续不展示封面。
- 修改 `src/features/mods/ModLibraryPage.css`
  - 保持 `.mod-card__poster` 固定高度或固定比例，不因图片加载改变布局。
  - 新增 `.mod-card__poster-img` 的 `position: absolute; inset: 0; width: 100%; height: 100%; object-fit: cover; object-position: center top;`。
- 创建 `src/features/mods/modPreviewImage.test.mjs`
  - 用静态 CSS/TSX 检查卡片图像类、懒加载、失败回退和布局约束。

## 实现任务

### Task 1: 领域模型和策略

**Files:**

- Create: `src-tauri/crates/hmm-core/src/preview_image.rs`
- Modify: `src-tauri/crates/hmm-core/src/lib.rs`
- Test: `src-tauri/crates/hmm-core/src/preview_image.rs`

- [ ] **Step 1: 写领域类型和默认策略测试**

在 `src-tauri/crates/hmm-core/src/preview_image.rs` 中先写测试，覆盖默认策略：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_is_conservative() {
        let policy = PreviewImagePolicy::default();

        assert_eq!(policy.max_input_bytes, 20 * 1024 * 1024);
        assert_eq!(policy.max_decoded_pixels, 16_000_000);
        assert_eq!(policy.max_candidates_per_package, 8);
        assert_eq!(policy.output_max_edge_px, 768);
        assert_eq!(policy.output_quality, 80);
        assert_eq!(policy.preferred_output_format, PreviewImageOutputFormat::Jpeg);
        assert!(policy.validate().is_ok());
    }

    #[test]
    fn rejects_zero_limits() {
        let mut policy = PreviewImagePolicy::default();
        policy.output_max_edge_px = 0;

        assert_eq!(
            policy.validate(),
            Err(PreviewImagePolicyError::InvalidOutputMaxEdge)
        );
    }
}
```

- [ ] **Step 2: 实现领域类型**

实现 `PreviewImagePolicy`、`PreviewImageOutputFormat`、`PreviewImageStatus`、`PreviewImageRejectionReason` 和 `PreviewImagePolicyError`。enum 序列化值使用稳定 `snake_case` 字符串，便于 DTO 映射。

```rust
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewImageOutputFormat {
    WebP,
    Jpeg,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewImagePolicy {
    pub max_input_bytes: u64,
    pub max_decoded_pixels: u64,
    pub max_candidates_per_package: usize,
    pub output_max_edge_px: u32,
    pub output_quality: u8,
    pub preferred_output_format: PreviewImageOutputFormat,
}

impl Default for PreviewImagePolicy {
    fn default() -> Self {
        Self {
            max_input_bytes: 20 * 1024 * 1024,
            max_decoded_pixels: 16_000_000,
            max_candidates_per_package: 8,
            output_max_edge_px: 768,
            output_quality: 80,
            preferred_output_format: PreviewImageOutputFormat::Jpeg,
        }
    }
}

impl PreviewImagePolicy {
    pub fn validate(&self) -> Result<(), PreviewImagePolicyError> {
        if self.max_input_bytes == 0 {
            return Err(PreviewImagePolicyError::InvalidMaxInputBytes);
        }
        if self.max_decoded_pixels == 0 {
            return Err(PreviewImagePolicyError::InvalidPixelLimit);
        }
        if self.max_candidates_per_package == 0 {
            return Err(PreviewImagePolicyError::InvalidCandidateLimit);
        }
        if self.output_max_edge_px == 0 {
            return Err(PreviewImagePolicyError::InvalidOutputMaxEdge);
        }
        if !(1..=100).contains(&self.output_quality) {
            return Err(PreviewImagePolicyError::InvalidOutputQuality);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewImageRejectionReason {
    Missing,
    TooLarge,
    TooManyCandidates,
    UnsupportedFormat,
    DecodeFailed,
    PixelLimitExceeded,
    CacheWriteFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewImageStatus {
    Thumbnail,
    Fallback,
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum PreviewImagePolicyError {
    #[error("max input bytes must be greater than zero")]
    InvalidMaxInputBytes,
    #[error("pixel limit must be greater than zero")]
    InvalidPixelLimit,
    #[error("candidate limit must be greater than zero")]
    InvalidCandidateLimit,
    #[error("output max edge must be greater than zero")]
    InvalidOutputMaxEdge,
    #[error("output quality must be between 1 and 100")]
    InvalidOutputQuality,
}
```

- [ ] **Step 3: re-export 并运行测试**

在 `src-tauri/crates/hmm-core/src/lib.rs` 增加：

```rust
mod preview_image;

pub use preview_image::{
    PreviewImageOutputFormat, PreviewImagePolicy, PreviewImagePolicyError,
    PreviewImageRejectionReason, PreviewImageStatus,
};
```

运行：

```powershell
cargo test -p hmm-core preview_image
```

预期：新增测试通过。

- [ ] **Step 4: 提交**

```powershell
git add src-tauri/crates/hmm-core/src/lib.rs src-tauri/crates/hmm-core/src/preview_image.rs
git commit -m "feat(core): add preview image policy model"
```

### Task 2: Ports 接口

**Files:**

- Create: `src-tauri/crates/hmm-ports/src/preview_image.rs`
- Modify: `src-tauri/crates/hmm-ports/src/lib.rs`

- [ ] **Step 1: 定义内部引用和值对象**

在 `src-tauri/crates/hmm-ports/src/preview_image.rs` 定义：

```rust
use anyhow::Result;
use hmm_core::{PreviewImagePolicy, PreviewImageRejectionReason};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PreviewImageSourceRef {
    pub package_id: String,
    pub logical_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewImageCandidate {
    pub source_ref: PreviewImageSourceRef,
    pub file_name: String,
    pub compressed_size: u64,
    pub priority: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ThumbnailRef {
    pub package_id: String,
    pub content_hash: String,
    pub variant: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessedPreviewImage {
    pub thumbnail_ref: ThumbnailRef,
    pub width: u32,
    pub height: u32,
    pub content_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviewImageProcessingResult {
    Thumbnail(ProcessedPreviewImage),
    Fallback(PreviewImageRejectionReason),
}

pub trait PackagePreviewScanner: Send + Sync {
    fn scan_candidates(
        &self,
        package_id: &str,
        sandbox_root: &Path,
        policy: &PreviewImagePolicy,
    ) -> Result<Vec<PreviewImageCandidate>>;
}

pub trait PreviewImageProcessor: Send + Sync {
    fn process_candidate(
        &self,
        sandbox_root: &Path,
        candidate: &PreviewImageCandidate,
        policy: &PreviewImagePolicy,
    ) -> Result<PreviewImageProcessingResult>;
}

pub trait ThumbnailStore: Send + Sync {
    fn put_thumbnail(
        &self,
        package_id: &str,
        content_hash: &str,
        extension: &str,
        bytes: &[u8],
    ) -> Result<ThumbnailRef>;

    fn resolve_url(&self, thumbnail_ref: &ThumbnailRef) -> Result<String>;
}
```

约束：

- `sandbox_root` 只来自后端导入流水线，不来自前端。
- `logical_path` 只能是包内逻辑路径，不能是本地绝对路径。
- `ThumbnailRef` 是 opaque reference，前端不直接看到真实磁盘路径。

- [ ] **Step 2: re-export ports**

在 `src-tauri/crates/hmm-ports/src/lib.rs` 增加：

```rust
mod preview_image;

pub use preview_image::{
    PackagePreviewScanner, PreviewImageCandidate, PreviewImageProcessingResult,
    PreviewImageProcessor, PreviewImageSourceRef, ProcessedPreviewImage, ThumbnailRef,
    ThumbnailStore,
};
```

- [ ] **Step 3: 运行编译检查**

```powershell
cargo check -p hmm-ports
```

预期：`hmm-ports` 编译通过。

- [ ] **Step 4: 提交**

```powershell
git add src-tauri/crates/hmm-ports/src/lib.rs src-tauri/crates/hmm-ports/src/preview_image.rs
git commit -m "feat(ports): define preview image ports"
```

### Task 3: Infra magic bytes 与候选扫描

**Files:**

- Create: `src-tauri/crates/hmm-infra/src/preview_image/mod.rs`
- Create: `src-tauri/crates/hmm-infra/src/preview_image/magic_bytes.rs`
- Create: `src-tauri/crates/hmm-infra/src/preview_image/scanner.rs`
- Modify: `src-tauri/crates/hmm-infra/src/lib.rs`
- Test: `src-tauri/crates/hmm-infra/src/preview_image/magic_bytes.rs`
- Test: `src-tauri/crates/hmm-infra/src/preview_image/scanner.rs`

- [ ] **Step 1: 写 magic bytes 测试**

覆盖 PNG、JPEG、WebP 和伪装文件：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_supported_magic_bytes() {
        assert_eq!(detect_image_format(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]), Some(DetectedImageFormat::Png));
        assert_eq!(detect_image_format(&[0xff, 0xd8, 0xff, 0xdb]), Some(DetectedImageFormat::Jpeg));
        assert_eq!(detect_image_format(b"RIFF\x20\x00\x00\x00WEBP"), Some(DetectedImageFormat::WebP));
    }

    #[test]
    fn rejects_text_with_image_extension() {
        assert_eq!(detect_image_format(b"not actually an image"), None);
    }
}
```

- [ ] **Step 2: 实现 magic bytes**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectedImageFormat {
    Png,
    Jpeg,
    WebP,
}

pub fn detect_image_format(bytes: &[u8]) -> Option<DetectedImageFormat> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
        return Some(DetectedImageFormat::Png);
    }
    if bytes.len() >= 3 && bytes[0..3] == [0xff, 0xd8, 0xff] {
        return Some(DetectedImageFormat::Jpeg);
    }
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Some(DetectedImageFormat::WebP);
    }
    None
}
```

- [ ] **Step 3: 实现候选扫描器**

扫描器只遍历安全解压后的 sandbox，稳定排序，限制候选数量：

```rust
use anyhow::Result;
use hmm_core::PreviewImagePolicy;
use hmm_ports::{PackagePreviewScanner, PreviewImageCandidate, PreviewImageSourceRef};
use std::path::Path;

pub struct SandboxPackagePreviewScanner;

impl PackagePreviewScanner for SandboxPackagePreviewScanner {
    fn scan_candidates(
        &self,
        package_id: &str,
        sandbox_root: &Path,
        policy: &PreviewImagePolicy,
    ) -> Result<Vec<PreviewImageCandidate>> {
        let mut candidates = Vec::new();
        collect_candidates(package_id, sandbox_root, sandbox_root, &mut candidates)?;
        candidates.sort_by_key(|candidate| {
            (
                candidate.priority,
                candidate.source_ref.logical_path.to_ascii_lowercase(),
            )
        });
        candidates.truncate(policy.max_candidates_per_package);
        Ok(candidates)
    }
}

fn collect_candidates(
    package_id: &str,
    sandbox_root: &Path,
    current_dir: &Path,
    out: &mut Vec<PreviewImageCandidate>,
) -> Result<()> {
    for entry in std::fs::read_dir(current_dir)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            continue;
        }
        let path = entry.path();
        if file_type.is_dir() {
            collect_candidates(package_id, sandbox_root, &path, out)?;
            continue;
        }
        if !file_type.is_file() || !has_image_extension(&path) {
            continue;
        }

        let relative = path.strip_prefix(sandbox_root)?;
        let logical_path = relative.to_string_lossy().replace('\\', "/");
        let file_name = entry.file_name().to_string_lossy().to_string();
        let compressed_size = entry.metadata()?.len();

        out.push(PreviewImageCandidate {
            source_ref: PreviewImageSourceRef {
                package_id: package_id.to_string(),
                logical_path,
            },
            priority: candidate_priority(&file_name),
            file_name,
            compressed_size,
        });
    }

    Ok(())
}

fn has_image_extension(path: &Path) -> bool {
    let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
        return false;
    };
    matches!(
        extension.to_ascii_lowercase().as_str(),
        "png" | "jpg" | "jpeg" | "webp"
    )
}

fn candidate_priority(file_name: &str) -> u16 {
    let stem = std::path::Path::new(file_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    match stem.as_str() {
        "preview" => 0,
        "cover" => 1,
        "poster" => 2,
        "thumbnail" => 3,
        "image" => 4,
        _ => 10,
    }
}
```

实现细节：

- 扩展名只用于发现候选，不作为格式可信依据。
- 文件名优先级：`preview`、`cover`、`poster`、`thumbnail`、`image` 高于普通图片。
- 逻辑路径统一使用 `/` 分隔。
- 如果发现符号链接、目录联接或无法 canonicalize 的条目，跳过并记录 recoverable 诊断；诊断只记录结构化原因码和稳定 id，不记录真实本地路径、sandbox 布局或压缩包内部原始路径。
- 不读取 sandbox 外路径。

- [ ] **Step 4: 测试候选排序和数量限制**

测试使用临时目录创建最小空文件，只验证扫描排序和数量限制，不做解码：

```rust
#[test]
fn scanner_prefers_preview_names_and_limits_count() {
    let temp = tempfile::tempdir().expect("temp dir");
    std::fs::write(temp.path().join("zzz.jpg"), b"").expect("write zzz");
    std::fs::write(temp.path().join("preview.png"), b"").expect("write preview");
    std::fs::write(temp.path().join("cover.webp"), b"").expect("write cover");

    let mut policy = PreviewImagePolicy::default();
    policy.max_candidates_per_package = 2;

    let scanner = SandboxPackagePreviewScanner;
    let candidates = scanner
        .scan_candidates("pkg-1", temp.path(), &policy)
        .expect("scan candidates");

    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0].file_name, "preview.png");
    assert_eq!(candidates[1].file_name, "cover.webp");
}
```

- [ ] **Step 5: 增加 dev/test 依赖并运行测试**

如果使用 `tempfile`，在 `src-tauri/crates/hmm-infra/Cargo.toml` 的 `[dev-dependencies]` 加入：

```toml
tempfile = "3"
```

运行：

```powershell
cargo test -p hmm-infra preview_image
```

预期：magic bytes 和 scanner 测试通过。

- [ ] **Step 6: 提交**

```powershell
git add src-tauri/crates/hmm-infra/Cargo.toml src-tauri/crates/hmm-infra/src/lib.rs src-tauri/crates/hmm-infra/src/preview_image
git commit -m "feat(infra): scan preview image candidates"
```

### Task 4: 图片处理器与缩略图缓存

**Files:**

- Create: `src-tauri/crates/hmm-infra/src/preview_image/processor.rs`
- Create: `src-tauri/crates/hmm-infra/src/preview_image/thumbnail_store.rs`
- Modify: `src-tauri/crates/hmm-infra/src/preview_image/mod.rs`
- Modify: `src-tauri/crates/hmm-infra/Cargo.toml`
- Test: `src-tauri/crates/hmm-infra/src/preview_image/processor.rs`
- Test: `src-tauri/crates/hmm-infra/src/preview_image/thumbnail_store.rs`

- [ ] **Step 1: 引入图片处理依赖**

在 `src-tauri/crates/hmm-infra/Cargo.toml` 增加：

```toml
image = { version = "0.25.10", default-features = false, features = ["jpeg", "png", "webp"] }
sha2 = "0.10"
```

说明：

- `image` 用于读取 header、解码和缩放。
- `sha2` 用于缓存文件名的稳定内容 hash。
- 当前 MVP 保持 WebP 解码支持，默认输出格式先落 JPEG，以便稳定使用 `output_quality`；后续如果引入稳定有损 WebP 编码器，再把 `preferred_output_format` 默认值切回 WebP。

- [ ] **Step 2: 写处理器安全测试**

测试必须人工生成最小图片，不提交真实 Mod 包：

```rust
#[test]
fn rejects_candidate_over_input_size_limit() {
    let temp = tempfile::tempdir().expect("temp dir");
    let image_path = temp.path().join("preview.png");
    std::fs::write(&image_path, vec![0_u8; 128]).expect("write fake image");

    let candidate = preview_candidate("pkg-1", "preview.png", 128);
    let mut policy = PreviewImagePolicy::default();
    policy.max_input_bytes = 64;

    let processor = ImageCratePreviewImageProcessor::new(InMemoryThumbnailStore::default());
    let result = processor
        .process_candidate(temp.path(), &candidate, &policy)
        .expect("processing result");

    assert_eq!(
        result,
        PreviewImageProcessingResult::Fallback(PreviewImageRejectionReason::TooLarge)
    );
}
```

测试文件内补充明确 helper，避免测试依赖真实导入流水线：

```rust
fn preview_candidate(package_id: &str, logical_path: &str, compressed_size: u64) -> PreviewImageCandidate {
    PreviewImageCandidate {
        source_ref: PreviewImageSourceRef {
            package_id: package_id.to_string(),
            logical_path: logical_path.to_string(),
        },
        file_name: logical_path.rsplit('/').next().unwrap_or(logical_path).to_string(),
        compressed_size,
        priority: 0,
    }
}

#[derive(Default)]
struct InMemoryThumbnailStore {
    last_bytes: std::sync::Mutex<Option<Vec<u8>>>,
}

impl ThumbnailStore for InMemoryThumbnailStore {
    fn put_thumbnail(
        &self,
        package_id: &str,
        content_hash: &str,
        _extension: &str,
        bytes: &[u8],
    ) -> anyhow::Result<ThumbnailRef> {
        *self.last_bytes.lock().expect("thumbnail store lock") = Some(bytes.to_vec());
        Ok(ThumbnailRef {
            package_id: package_id.to_string(),
            content_hash: content_hash.to_string(),
            variant: "preview-768".to_string(),
        })
    }

    fn resolve_url(&self, thumbnail_ref: &ThumbnailRef) -> anyhow::Result<String> {
        Ok(format!(
            "thumbnail://{}/{}/{}",
            thumbnail_ref.package_id, thumbnail_ref.variant, thumbnail_ref.content_hash
        ))
    }
}
```

继续覆盖：

- magic bytes 不匹配返回 `UnsupportedFormat`。
- 损坏图片返回 `DecodeFailed`。
- 像素数超过 `max_decoded_pixels` 返回 `PixelLimitExceeded`。
- 正常 PNG/JPEG 生成最长边不超过 `output_max_edge_px` 的缩略图。

- [ ] **Step 3: 实现便宜检查优先**

处理顺序固定为：

```text
resolve candidate under sandbox
-> metadata length <= max_input_bytes
-> read first 16 bytes and check magic bytes
-> image::ImageReader::with_guessed_format
-> read dimensions
-> width * height <= max_decoded_pixels
-> decode
-> resize preserving aspect ratio
-> encode to policy output format or safe fallback format
-> store.put_thumbnail
```

实现要求：

- 使用 `checked_mul` 计算像素数，溢出直接 `PixelLimitExceeded`。
- 任何 I/O 错误不能 panic，转换成 fallback 或 recoverable error。
- 解码前不一次性读取超大文件到内存。
- 输出缩略图宽高来自实际编码前的 resized image。
- 缩略图内容 hash 基于输出 bytes，而不是原始文件名。

- [ ] **Step 4: 实现原子写入缓存**

`FileSystemThumbnailStore` 只写应用数据目录下的缩略图缓存：

```text
<app_data_dir>/thumbnails/<package_id>/preview-<content_hash>-768.<ext>
```

规则：

- `package_id` 和 `content_hash` 进入路径前必须经过字符白名单或编码。
- 先写 `.tmp` 文件，再 rename 成最终文件。
- 如果最终文件已存在且 hash 相同，可以直接返回 `ThumbnailRef`。
- `resolve_url` 返回受控 URL 或 `thumbnail://<package_id>/<variant>/<content_hash>` 这类 opaque URL，不返回真实磁盘路径。

- [ ] **Step 5: 运行 infra 测试**

```powershell
cargo test -p hmm-infra preview_image
```

预期：正常图、伪装图、损坏图、超限图和缓存写入测试通过。

- [ ] **Step 6: 提交**

```powershell
git add src-tauri/crates/hmm-infra/Cargo.toml src-tauri/crates/hmm-infra/src/preview_image
git commit -m "feat(infra): process preview image thumbnails safely"
```

### Task 5: App 编排服务

**Files:**

- Create: `src-tauri/crates/hmm-app/src/preview_image.rs`
- Modify: `src-tauri/crates/hmm-app/src/lib.rs`
- Test: `src-tauri/crates/hmm-app/src/preview_image.rs`

- [ ] **Step 1: 写服务行为测试**

使用 fake scanner、fake processor、fake store，不碰真实文件系统：

```rust
#[test]
fn returns_missing_fallback_when_no_candidates_exist() {
    let service = PreviewImageService::new(
        PreviewImagePolicy::default(),
        Box::new(FakeScanner::new(vec![])),
        Box::new(FakeProcessor::unused()),
        Box::new(FakeThumbnailStore::default()),
    );

    let result = service
        .process_package_preview("task-1", "pkg-1", Path::new("sandbox"))
        .expect("preview result");

    assert_eq!(
        result,
        PreviewImageProcessingResult::Fallback(PreviewImageRejectionReason::Missing)
    );
}
```

继续覆盖：

- 第一个候选失败后尝试下一个候选。
- 全部候选失败后返回最后一个稳定 fallback reason。
- 候选超过限制时不处理超出部分。
- 缓存 URL 解析失败时返回 `CacheWriteFailed` fallback。

- [ ] **Step 2: 实现 `PreviewImageService`**

服务职责：

- 校验 `PreviewImagePolicy`。
- 调用 scanner 获取候选。
- 对候选按顺序调用 processor。
- 成功时返回 thumbnail 结果。
- 失败时返回 fallback，不抛出会中断导入的错误，除非策略本身非法或 sandbox 不可访问。
- 记录 task 级结构化事件时只带 `task_id`、`package_id`、候选数量、结果、原因、耗时，不带真实路径。

- [ ] **Step 3: re-export 并运行测试**

```powershell
cargo test -p hmm-app preview_image
```

预期：服务编排测试通过。

- [ ] **Step 4: 提交**

```powershell
git add src-tauri/crates/hmm-app/src/lib.rs src-tauri/crates/hmm-app/src/preview_image.rs
git commit -m "feat(app): orchestrate preview image processing"
```

### Task 6: DTO 和前后端契约接入

**Files:**

- Modify: `src-tauri/src/dto.rs`
- Later import command integration: `src-tauri/src/mod_import_commands.rs` or the concrete import command file introduced by the Mod import task
- Test: `src-tauri/src/dto.rs`

- [ ] **Step 1: 定义 DTO**

DTO 形状与 [前后端通信契约设计](FRONTEND_BACKEND_CONTRACT.md) 保持一致：

```rust
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", rename_all_fields = "camelCase")]
pub enum PreviewImageDto {
    Thumbnail {
        thumbnail_url: String,
        width: u32,
        height: u32,
        content_hash: String,
    },
    Fallback {
        reason: PreviewImageFallbackReasonDto,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewImageFallbackReasonDto {
    Missing,
    TooLarge,
    TooManyCandidates,
    UnsupportedFormat,
    DecodeFailed,
    PixelLimitExceeded,
    CacheWriteFailed,
}
```

注意：Rust 字段可用 snake_case，但类型必须通过 serde 输出 camelCase；如果 enum 内字段不能自动满足，需要改成 struct variant 或手写转换测试。

- [ ] **Step 2: 写序列化测试**

```rust
#[test]
fn serializes_thumbnail_dto_with_camel_case_fields() {
    let dto = PreviewImageDto::Thumbnail {
        thumbnail_url: "thumbnail://pkg/preview/hash".to_string(),
        width: 512,
        height: 768,
        content_hash: "abc123".to_string(),
    };

    let value = serde_json::to_value(dto).expect("serialize dto");

    assert_eq!(value["kind"], "thumbnail");
    assert_eq!(value["thumbnailUrl"], "thumbnail://pkg/preview/hash");
    assert_eq!(value["contentHash"], "abc123");
}
```

- [ ] **Step 3: 接入 import/library DTO**

当前仓库还没有完整 Mod 导入 command。实现 Mod 导入任务时按以下契约接入：

- `start_import_mod_task` 只启动任务，返回 `TaskStartedDto`。
- `get_mod_library` 和 `get_mod_detail` 返回 `previewImage: PreviewImageDto`。
- 导入任务事件带 `taskId` 和阶段 message code，例如 `mod_import.preview_image.processing`、`mod_import.preview_image.fallback`。
- 不把 `ThumbnailRef`、缓存目录、sandbox 路径暴露给前端。
- 在真实 command 落地前，Tauri DTO 层已提供 `ImportPreviewImage -> PreviewImageDto` 映射，后续 command 不需要重复拼接 `thumbnailUrl`、尺寸和 fallback reason。
- 当前已落地 `start_import_mod_task` 最小入口：只校验 archive 路径、生成 `taskId` 并返回 `TaskStartedDto`；安全解压、任务事件、`ModImportAnalysisService` 接线和 library/detail 查询仍是后续任务。

- [ ] **Step 4: 运行 Tauri/Rust 检查**

```powershell
cargo test -p hmm-tauri preview_image
cargo check --workspace
```

预期：DTO 序列化测试通过，workspace 编译通过。

- [ ] **Step 5: 提交**

```powershell
git add src-tauri/src/dto.rs
git commit -m "feat(tauri): add preview image dto"
```

### Task 7: 前端类型和卡片展示

**Files:**

- Create: `src/features/mods/modPreviewImageTypes.ts`
- Modify: `src/features/mods/modsLibraryData.ts`
- Modify: `src/features/mods/ModPosterCard.tsx`
- Modify: `src/features/mods/ModLibraryPage.css`
- Create: `src/features/mods/modPreviewImage.test.mjs`

- [ ] **Step 1: 定义前端类型**

```ts
export type PreviewImage =
  | {
      kind: "thumbnail";
      thumbnailUrl: string;
      width: number;
      height: number;
      contentHash: string;
    }
  | {
      kind: "fallback";
      reason:
        | "missing"
        | "too_large"
        | "too_many_candidates"
        | "unsupported_format"
        | "decode_failed"
        | "pixel_limit_exceeded"
        | "cache_write_failed";
    };
```

- [ ] **Step 2: 扩展 `ModLibraryItem`**

在 `modsLibraryData.ts` 中加入：

```ts
import type { PreviewImage } from "./modPreviewImageTypes";

export type ModLibraryItem = {
  id: string;
  name: string;
  status: "installed" | "disabled" | "conflict";
  sizeLabel: string;
  posterFrom: string;
  posterTo: string;
  author?: string;
  versionLabel?: string;
  previewImage?: PreviewImage;
};
```

mock 数据不引入真实图片；默认 `previewImage` 可省略或设为 `{ kind: "fallback", reason: "missing" }`。

- [ ] **Step 3: 修改卡片渲染**

在 `ModPosterCard.tsx` 中增加本地加载失败状态：

```tsx
import { useState } from "react";

const [posterFailed, setPosterFailed] = useState(false);
const canShowPoster = item.previewImage?.kind === "thumbnail" && !posterFailed;
```

封面区域中在剪影之前渲染：

```tsx
{canShowPoster && (
  <img
    className="mod-card__poster-img"
    src={item.previewImage.thumbnailUrl}
    loading="lazy"
    decoding="async"
    alt=""
    onError={() => setPosterFailed(true)}
  />
)}
```

剪影占位保留，并在有图时由 CSS 降低或隐藏可见性：

```tsx
<span className="mod-card__silhouette" aria-hidden="true" data-visible={!canShowPoster}>
  ...
</span>
```

要求：

- tech 视图继续不展示封面。
- 图片失败只影响当前卡片展示，不修改后端状态。
- `thumbnailUrl` 只来自后端 DTO 或 mock，不由前端拼路径。

- [ ] **Step 4: 补充 CSS**

```css
.mod-card__poster {
  position: relative;
  overflow: hidden;
}

.mod-card__poster-img {
  position: absolute;
  inset: 0;
  z-index: 0;
  width: 100%;
  height: 100%;
  object-fit: cover;
  object-position: center top;
}

.mod-card__poster-img + .mod-card__silhouette {
  opacity: 0;
}

.mod-card__silhouette[data-visible="true"] {
  opacity: 1;
}
```

保留当前卡片渐变、徽标和选中态层级，确保状态徽标在图片上方。

- [ ] **Step 5: 增加静态回归测试**

`modPreviewImage.test.mjs` 检查：

- `ModPosterCard.tsx` 包含 `loading="lazy"`。
- `ModPosterCard.tsx` 包含 `decoding="async"`。
- `ModPosterCard.tsx` 包含 `onError` fallback。
- CSS 中 `.mod-card__poster-img` 包含 `object-fit: cover`、`position: absolute`、`inset: 0`。

- [ ] **Step 6: 运行前端检查**

```powershell
cmd /c corepack pnpm run typecheck
cmd /c corepack pnpm run lint
cmd /c corepack pnpm run test
```

预期：类型检查、lint 和前端测试通过。

- [ ] **Step 7: 提交**

```powershell
git add src/features/mods/modPreviewImageTypes.ts src/features/mods/modsLibraryData.ts src/features/mods/ModPosterCard.tsx src/features/mods/ModLibraryPage.css src/features/mods/modPreviewImage.test.mjs
git commit -m "feat(ui): render controlled mod preview thumbnails"
```

### Task 8: 统一验证和文档同步

**Files:**

- Review and modify if DTO/output behavior changed: `docs/FRONTEND_BACKEND_CONTRACT.md`
- Review and modify if output format defaults changed: `docs/MOD_PREVIEW_IMAGE_PIPELINE_DESIGN.md`
- Review and modify if verification commands changed: `docs/TESTING.md`
- Review and modify if new docs were added: `README.md`

- [ ] **Step 1: 契约差异检查**

确认实现中的 DTO 字段与契约一致：

```text
kind
thumbnailUrl
width
height
contentHash
reason
```

如果实现选择 JPEG 作为 MVP 输出格式，更新设计文档中的默认策略说明：JPEG 是当前经过验证的安全默认输出格式，WebP 保留为后续可选优化。

- [ ] **Step 2: 安全清单检查**

逐项确认：

- 没有新增宽泛 Tauri 文件读取 command。
- 前端没有拼接本地图片路径。
- 日志没有完整路径或第三方图片内容。
- 图片处理不在游戏写锁内执行。
- 缩略图缓存可删除、可重建，不作为安装事实来源。
- 测试数据全部为人工构造的最小样本。

- [ ] **Step 3: 运行统一验证**

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

预期：统一验证通过。

- [ ] **Step 4: 提交文档同步**

```powershell
git add README.md docs/FRONTEND_BACKEND_CONTRACT.md docs/MOD_PREVIEW_IMAGE_PIPELINE_DESIGN.md docs/TESTING.md
git commit -m "docs: sync preview image implementation notes"
```

## 验收标准

实现完成后必须满足：

- 正常 PNG/JPEG/WebP 候选图可以生成受控缩略图。
- 超大压缩态文件不进入解码阶段。
- magic bytes 不匹配的伪装图片返回 fallback。
- 损坏图片返回 fallback，不 panic。
- 解码后像素数超过限制返回 fallback。
- 候选图数量超过限制时只处理前 `max_candidates_per_package` 个稳定排序候选。
- 缩略图缓存写入失败时导入主流程仍可返回 fallback。
- 前端卡片在 thumbnail、fallback、图片加载失败三种状态下尺寸不跳动。
- `PreviewImageDto` 序列化字段与 TypeScript 类型一致。
- 任务进度事件携带 `taskId`。
- 日志不包含完整本地路径、第三方图片内容或敏感信息。
- `scripts/verify.ps1` 通过，或在 PR 中明确记录未执行项及原因。

## 实施顺序建议

1. 先实现 Task 1 和 Task 2，锁定跨 crate 类型边界。
2. 再实现 Task 3 和 Task 4，用最小图片样本压实安全处理。
3. 然后实现 Task 5，让 app 层在 fake ports 下稳定编排。
4. 接着实现 Task 6，把后端结果转成前端可消费 DTO。
5. 最后实现 Task 7，让 UI 展示受控缩略图并保留 fallback。
6. Task 8 在 PR 收尾阶段执行，确保契约和验证记录同步。

## 风险与取舍

- WebP 输出体积更优，但 Rust 编码链和目标平台支持需要验证。实现时应让输出格式可配置，MVP 可用 JPEG 作为安全退回。
- 图片 header 读取和完整解码之间仍有内存风险，必须先检查文件大小、magic bytes 和像素数，并限制图片处理并发。
- 缩略图缓存不是安全事实来源，缓存损坏或缺失只能影响展示，不能影响安装、卸载或回滚。
- fallback 原因用于用户提示和测试，不用于前端推断底层文件系统状态。
- 预览图处理依赖安全解压后的 sandbox；在完整导入流水线落地前，可以先用 app service 和 infra 单元测试验证模块边界。
