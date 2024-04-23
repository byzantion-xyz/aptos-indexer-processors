CREATE TABLE IF NOT EXISTS accounts (
  account_address VARCHAR(66) NOT NULL,
  inserted_at TIMESTAMP NOT NULL DEFAULT NOW(),
  PRIMARY KEY (account_address)
);