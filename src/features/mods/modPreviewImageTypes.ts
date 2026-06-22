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
