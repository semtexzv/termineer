-- Add Google OAuth token fields to users table

-- Add new columns for Google OAuth tokens
ALTER TABLE users ADD COLUMN google_access_token TEXT;
ALTER TABLE users ADD COLUMN google_refresh_token TEXT;
ALTER TABLE users ADD COLUMN token_expires_at TIMESTAMPTZ;

-- Add index for faster token lookups
CREATE INDEX idx_users_auth_provider_id ON users(auth_provider_id);