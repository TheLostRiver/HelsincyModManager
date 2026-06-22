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
            preferred_output_format: PreviewImageOutputFormat::WebP,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
        assert_eq!(
            policy.preferred_output_format,
            PreviewImageOutputFormat::WebP
        );
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

    #[test]
    fn serializes_rejection_reason_as_snake_case() {
        let value = serde_json::to_string(&PreviewImageRejectionReason::PixelLimitExceeded)
            .expect("reason should serialize");

        assert_eq!(value, r#""pixel_limit_exceeded""#);
    }
}
