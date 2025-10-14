-- Create account_balance_changes table
CREATE TABLE IF NOT EXISTS account_balance_changes (
    id BIGSERIAL PRIMARY KEY,
    transaction_id BIGINT NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
    account_address VARCHAR(44) NOT NULL,
    mint_address VARCHAR(44),
    pre_balance BIGINT NOT NULL,
    post_balance BIGINT NOT NULL,
    balance_delta BIGINT NOT NULL
);

-- Create indexes for common query patterns
CREATE INDEX IF NOT EXISTS idx_balance_changes_transaction_id ON account_balance_changes(transaction_id);
CREATE INDEX IF NOT EXISTS idx_balance_changes_account_address ON account_balance_changes(account_address);
CREATE INDEX IF NOT EXISTS idx_balance_changes_mint_address ON account_balance_changes(mint_address);
