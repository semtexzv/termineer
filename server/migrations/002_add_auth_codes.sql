-- Add a table for one-time authentication codes
-- These codes allow users who have authenticated on the website
-- to authenticate CLI sessions without going through OAuth again

CREATE TABLE IF NOT EXISTS auth_codes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    code VARCHAR(12) NOT NULL, -- Short, readable code
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL,
    used BOOLEAN NOT NULL DEFAULT FALSE,
    CONSTRAINT unique_code UNIQUE (code)
);

CREATE INDEX IF NOT EXISTS auth_codes_user_id_idx ON auth_codes(user_id);
CREATE INDEX IF NOT EXISTS auth_codes_code_idx ON auth_codes(code);

-- Add function to automatically clean up expired codes
CREATE OR REPLACE FUNCTION cleanup_expired_auth_codes() RETURNS TRIGGER AS $$
BEGIN
    DELETE FROM auth_codes WHERE expires_at < NOW();
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

-- Create trigger to clean up expired codes on insert
CREATE TRIGGER trigger_cleanup_expired_auth_codes
AFTER INSERT ON auth_codes
EXECUTE FUNCTION cleanup_expired_auth_codes();