CREATE TABLE IF NOT EXISTS register_tokens (
    id UUID PRIMARY KEY,
    expires_at TIMESTAMPTZ NOT NULL
);
