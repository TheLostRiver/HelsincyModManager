-- 历史查询派生列:权威事实仍是 batch_json / result_json,这两列只服务
-- 跨批次排序、状态聚合与保留期,可随时从 JSON 回填重建。
-- 刻意保持可空且不加 CHECK:启动期 migration 失败会直接砸掉应用,
-- 正常写路径由 Rust 枚举填值,残留 NULL 走读取侧的 result_json 兜底归类。
ALTER TABLE external_import_batches ADD COLUMN created_at INTEGER;

UPDATE external_import_batches
   SET created_at = json_extract(batch_json, '$.created_at_unix_millis');

CREATE INDEX idx_external_import_batches_history
    ON external_import_batches(created_at DESC, batch_id COLLATE BINARY ASC);

ALTER TABLE external_import_item_results ADD COLUMN status TEXT COLLATE BINARY;

UPDATE external_import_item_results
   SET status = json_extract(result_json, '$.status');

CREATE INDEX idx_external_import_item_results_status
    ON external_import_item_results(batch_id COLLATE BINARY, status COLLATE BINARY);
