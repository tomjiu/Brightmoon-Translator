-- Migration 003: Dictionary lookup history
-- Tracks per-word lookup count and timestamps for the dictionary history UI.
--
-- NOTE: This file is documentation-only. The actual table is created at runtime
-- by EventStore::init_schema() (see src/infrastructure/event_store.rs). The
-- project does not yet have a migration runner that executes .sql files at
-- startup; EventStore uses `CREATE TABLE IF NOT EXISTS` for idempotent schema
-- bootstrap. This file exists to document the schema shape alongside other
-- migrations (002/004/005 are also documentation-only; only 001 is loaded by
-- data_init.rs).

CREATE TABLE IF NOT EXISTS dictionary_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    word TEXT NOT NULL UNIQUE,
    lookup_count INTEGER NOT NULL DEFAULT 1,
    first_looked_up INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    last_looked_up INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_dictionary_history_last
    ON dictionary_history(last_looked_up DESC);
