-- Add a table for short-lived CLI access tokens
-- These tokens allow users who have authenticated on the website
-- to authenticate CLI sessions without going through OAuth again

CREATE TABLE IF NOT EXISTS cli_access_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL,
    used BOOLEAN NOT NULL DEFAULT FALSE,
    CONSTRAINT unique_token UNIQUE (token)
);

CREATE INDEX IF NOT EXISTS cli_access_tokens_user_id_idx ON cli_access_tokens(user_id);
CREATE INDEX IF NOT EXISTS cli_access_tokens_token_idx ON cli_access_tokens(token);

-- Add function to automatically clean up expired tokens
CREATE OR REPLACE FUNCTION cleanup_expired_cli_tokens() RETURNS TRIGGER AS $$
BEGIN
    DELETE FROM cli_access_tokens WHERE expires_at < NOW();
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

-- Create trigger to clean up expired tokens on insert
CREATE TRIGGER trigger_cleanup_expired_cli_tokens
AFTER INSERT ON cli_access_tokens
EXECUTE FUNCTION cleanup_expired_cli_tokens();