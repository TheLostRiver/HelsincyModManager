-- #286 切片 3b-1：列表页「外部来源」标记。
-- 投影是可重建读模型：加列后由 MOD_LIBRARY_PROJECTION_SCHEMA_VERSION（1→2）
-- 触发整体重建从权威目录补齐数据，本迁移不需要回填。
ALTER TABLE mod_library_projection_items
    ADD COLUMN external_import_adapter_id TEXT;
