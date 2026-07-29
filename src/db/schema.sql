-- Tabular Server — MySQL Schema
-- Run these migrations in order.

CREATE TABLE IF NOT EXISTS users (
    id            VARCHAR(36)  NOT NULL PRIMARY KEY DEFAULT (UUID()),
    provider      VARCHAR(20)  NOT NULL,            -- 'google' | 'github'
    provider_id   VARCHAR(255) NOT NULL,            -- OAuth subject/ID from provider
    email         VARCHAR(255) NOT NULL UNIQUE,
    display_name  VARCHAR(255),
    avatar_url    TEXT,
    created_at    DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at    DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    UNIQUE KEY uq_provider (provider, provider_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Sessions (refresh tokens)
CREATE TABLE IF NOT EXISTS sessions (
    id            VARCHAR(36)  NOT NULL PRIMARY KEY DEFAULT (UUID()),
    user_id       VARCHAR(36)  NOT NULL,
    refresh_token VARCHAR(512) NOT NULL UNIQUE,
    expires_at    DATETIME     NOT NULL,
    device_info   VARCHAR(500),
    created_at    DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    INDEX idx_sessions_user (user_id),
    INDEX idx_sessions_refresh (refresh_token(64))
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Connections (credential payload encrypted client-side with AES-256-GCM)
CREATE TABLE IF NOT EXISTS connections (
    id               VARCHAR(36)  NOT NULL PRIMARY KEY DEFAULT (UUID()),
    user_id          VARCHAR(36)  NOT NULL,
    name             VARCHAR(255) NOT NULL,
    db_type          VARCHAR(50)  NOT NULL,
    encrypted_config MEDIUMTEXT   NOT NULL,    -- base64(AES-256-GCM encrypted JSON)
    color_tag        VARCHAR(7),               -- optional hex color
    created_at       DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at       DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    INDEX idx_connections_user (user_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Query History
CREATE TABLE IF NOT EXISTS query_history (
    id              VARCHAR(36)  NOT NULL PRIMARY KEY DEFAULT (UUID()),
    user_id         VARCHAR(36)  NOT NULL,
    connection_name VARCHAR(255) NOT NULL,
    query_text      MEDIUMTEXT   NOT NULL,
    executed_at     DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    INDEX idx_history_user_time (user_id, executed_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Saved Queries (synced .sql files)
CREATE TABLE IF NOT EXISTS saved_queries (
    id              VARCHAR(36)  NOT NULL PRIMARY KEY DEFAULT (UUID()),
    user_id         VARCHAR(36)  NOT NULL,
    name            VARCHAR(255) NOT NULL,
    folder_path     VARCHAR(500) NOT NULL DEFAULT '/',
    query_text      MEDIUMTEXT   NOT NULL,
    connection_name VARCHAR(255),
    client_checksum VARCHAR(64),              -- SHA-256 of content for conflict detection
    created_at      DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at      DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    INDEX idx_queries_user (user_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Collaborative Rooms
CREATE TABLE IF NOT EXISTS collab_rooms (
    id              VARCHAR(36)  NOT NULL PRIMARY KEY DEFAULT (UUID()),
    owner_id        VARCHAR(36)  NOT NULL,
    name            VARCHAR(255) NOT NULL,
    description     TEXT,
    crdt_snapshot   MEDIUMBLOB,              -- Periodic Yjs doc snapshot (binary)
    snapshot_at     DATETIME,
    created_at      DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at      DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    FOREIGN KEY (owner_id) REFERENCES users(id) ON DELETE CASCADE,
    INDEX idx_rooms_owner (owner_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Room Members
CREATE TABLE IF NOT EXISTS room_members (
    room_id    VARCHAR(36) NOT NULL,
    user_id    VARCHAR(36) NOT NULL,
    role       VARCHAR(20) NOT NULL DEFAULT 'editor',  -- 'owner' | 'editor' | 'viewer'
    joined_at  DATETIME    NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (room_id, user_id),
    FOREIGN KEY (room_id) REFERENCES collab_rooms(id) ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    INDEX idx_room_members_user (user_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- OAuth state nonces (PKCE / CSRF protection)
CREATE TABLE IF NOT EXISTS oauth_states (
    state       VARCHAR(128) NOT NULL PRIMARY KEY,
    provider    VARCHAR(20)  NOT NULL,
    code_verifier VARCHAR(256),
    expires_at  DATETIME     NOT NULL,
    created_at  DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_oauth_expires (expires_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
