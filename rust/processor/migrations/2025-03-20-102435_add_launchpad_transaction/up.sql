CREATE TABLE IF NOT EXISTS launchpad_transactions (
  id VARCHAR(100) NOT NULL,
  timestamp BIGINT NOT NULL,
  sender VARCHAR(66) NOT NULL,
  payload JSONB NOT NULL,
  error_count INTEGER NOT NULL DEFAULT 0,
  error VARCHAR(500) NULL,
  PRIMARY KEY (id)
);