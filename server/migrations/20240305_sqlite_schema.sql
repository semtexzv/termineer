-- Initial schema for AutoSWE server (SQLite version)

-- Users table
CREATE TABLE users (
    id TEXT PRIMARY KEY,
    email TEXT NOT NULL UNIQUE,
    name TEXT,
    auth_provider TEXT NOT NULL DEFAULT 'google',
    auth_provider_id TEXT,
    is_active INTEGER NOT NULL DEFAULT 1,
    has_subscription INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Create index on email for faster lookups
CREATE INDEX idx_users_email ON users(email);

-- Subscriptions table
CREATE TABLE subscriptions (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    stripe_customer_id TEXT NOT NULL,
    stripe_subscription_id TEXT NOT NULL UNIQUE,
    plan_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'incomplete',
    current_period_start TEXT NOT NULL,
    current_period_end TEXT NOT NULL,
    cancel_at_period_end INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- Create indexes for faster lookups
CREATE INDEX idx_subscriptions_user_id ON subscriptions(user_id);
CREATE INDEX idx_subscriptions_stripe_subscription_id ON subscriptions(stripe_subscription_id);

-- License keys table
CREATE TABLE license_keys (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    license_key TEXT NOT NULL UNIQUE,
    is_active INTEGER NOT NULL DEFAULT 1,
    issued_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    last_verified_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- Create indexes for faster lookups
CREATE INDEX idx_license_keys_user_id ON license_keys(user_id);
CREATE INDEX idx_license_keys_license_key ON license_keys(license_key);

-- Usage statistics table for analytics
CREATE TABLE usage_stats (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    license_id TEXT,
    event_type TEXT NOT NULL,
    event_data TEXT NOT NULL DEFAULT '{}',
    client_version TEXT,
    client_platform TEXT,
    timestamp TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (license_id) REFERENCES license_keys(id) ON DELETE SET NULL
);

-- Create indexes for analytics queries
CREATE INDEX idx_usage_stats_user_id ON usage_stats(user_id);
CREATE INDEX idx_usage_stats_event_type ON usage_stats(event_type);
CREATE INDEX idx_usage_stats_timestamp ON usage_stats(timestamp);

-- Create triggers for automatic updated_at updates
CREATE TRIGGER update_users_modtime
    AFTER UPDATE ON users
    FOR EACH ROW
    BEGIN
        UPDATE users SET updated_at = datetime('now') WHERE id = NEW.id;
    END;

CREATE TRIGGER update_subscriptions_modtime
    AFTER UPDATE ON subscriptions
    FOR EACH ROW
    BEGIN
        UPDATE subscriptions SET updated_at = datetime('now') WHERE id = NEW.id;
    END;

CREATE TRIGGER update_license_keys_modtime
    AFTER UPDATE ON license_keys
    FOR EACH ROW
    BEGIN
        UPDATE license_keys SET updated_at = datetime('now') WHERE id = NEW.id;
    END;