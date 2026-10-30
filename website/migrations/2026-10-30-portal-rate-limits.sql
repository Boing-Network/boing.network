-- Issue #61 item 68: portal API request rate limiting.
CREATE TABLE IF NOT EXISTS portal_rate_limits (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  bucket_key TEXT NOT NULL,
  created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_portal_rate_limits_bucket_created
  ON portal_rate_limits(bucket_key, created_at);
