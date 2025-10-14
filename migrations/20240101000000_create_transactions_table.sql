-- Create transactions table
CREATE TABLE IF NOT EXISTS transactions (
    id BIGSERIAL PRIMARY KEY,
    signature VARCHAR(88) NOT NULL UNIQUE,
    slot BIGINT NOT NULL,
    block_time TIMESTAMP WITH TIME ZONE,
    fee BIGINT NOT NULL,
    fee_payer VARCHAR(44) NOT NULL,
    success BOOLEAN NOT NULL,
    compute_units_consumed BIGINT,
    ingested_at TIMESTAMP WITH TIME ZONE DEFAULT NOW() NOT NULL
);

-- Create indexes for common query patterns
CREATE INDEX IF NOT EXISTS idx_transactions_block_time ON transactions(block_time);
CREATE INDEX IF NOT EXISTS idx_transactions_slot ON transactions(slot);
CREATE INDEX IF NOT EXISTS idx_transactions_ingested_at ON transactions(ingested_at);
CREATE INDEX IF NOT EXISTS idx_transactions_fee_payer ON transactions(fee_payer);
