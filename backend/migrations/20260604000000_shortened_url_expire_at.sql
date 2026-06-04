ALTER TABLE shortened_urls
ADD expire_at INTEGER NOT NULL DEFAULT (unixepoch('now', '+1 day'));
