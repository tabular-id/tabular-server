# ⚡ Tabular Server (`tabular-server`)

[![Rust 2024](https://img.shields.io/badge/rust-2024-orange.svg)](https://www.rust-lang.org/)
[![Framework](https://img.shields.io/badge/framework-Axum_0.8-blue.svg)](https://github.com/tokio-rs/axum)
[![Database](https://img.shields.io/badge/database-MySQL_8.0-blue.svg)](https://www.mysql.com/)
[![CRDT](https://img.shields.io/badge/crdt-Yrs_(Yjs_Rust)-green.svg)](https://github.com/yjs/yrs)
[![License](https://img.shields.io/badge/license-MIT-purple.svg)](#license)

**Tabular Server** is the high-performance, asynchronous backend engine for the **Tabular** workspace—a modern, multi-database desktop & web client. Built with **Rust**, **Axum**, and **Tokio**, `tabular-server` powers secure user authentication, cloud synchronization of encrypted database profiles and query histories, and sub-millisecond real-time collaborative SQL editing using **CRDTs (Conflict-free Replicated Data Types)**.

---

## 🚀 Key Features

- **🔐 Robust OAuth2 & JWT Authentication**
  - Native OAuth2 authentication supporting **Google** and **GitHub** providers.
  - PKCE and CSRF nonces (`oauth_states`) for maximum authorization security.
  - Dual-token system: short-lived **JWT Access Tokens** and long-lived **Refresh Tokens** persisted securely in MySQL.

- **🛡️ Zero-Knowledge Encrypted Connection Sync**
  - Synchronizes user database connection configurations across devices.
  - Connection credentials are encrypted client-side using **AES-256-GCM** before transmission—the server only stores ciphertext (`encrypted_config`), ensuring zero-knowledge privacy.

- **⚡ Real-Time Collaborative SQL Editing (CRDT)**
  - Powered by **`yrs`** (the official Rust port of Yjs) over high-throughput **WebSockets**.
  - Multi-user real-time query editing with state-vector sync handshakes, delta broadcast channels, and automatic binary document snapshotting (`collab_rooms`).
  - Fine-grained room membership and role-based permissions (`owner`, `editor`, `viewer`).

- **📁 Cloud Sync for Saved Queries & Folders**
  - Cloud storage for `.sql` snippet collections organized in logical folder structures.
  - Client-side SHA-256 checksum validation for optimistic concurrency and conflict detection.

- **📜 Centralized Query Execution History**
  - User-scoped query execution logs for history search, auditability, and query reuse.

- **🛠️ High Reliability & Automatic Database Migrations**
  - Asynchronous MySQL connection pooling managed via **SQLx** with compile-time checked queries.
  - Embedded automatic database schema migrations executed on server launch.

---

## 🛠️ Tech Stack & Architecture

| Component | Technology | Description |
|---|---|---|
| **Language & Runtime** | [Rust](https://www.rust-lang.org/) (2024 Edition), [Tokio](https://tokio.rs/) | Multi-threaded asynchronous execution engine |
| **Web Framework** | [Axum 0.8](https://github.com/tokio-rs/axum) | Ergonomic, modular HTTP framework built on Hyper & Tower |
| **Database** | [MySQL 8.0](https://www.mysql.com/) via [SQLx 0.8](https://github.com/launchbadge/sqlx) | Pure async SQL driver with automatic connection pooling & migrations |
| **Real-Time CRDT** | [`yrs` 0.21](https://github.com/yjs/yrs) & WebSockets | Yjs document synchronization and broadcast channels |
| **Authentication** | `jsonwebtoken`, `argon2`, `reqwest` | JWT verification, token refresh loops, and OAuth token exchanges |
| **Encryption & Hashing** | `aes-gcm`, `sha2` | AES-256-GCM connection encryption and SHA-256 checksums |
| **Logging & Tracing** | `tracing`, `tracing-subscriber` | Structured logging with `EnvFilter` support |

---

## 📂 Project Structure

```
tabular-server/
├── .env.example           # Environment configuration template
├── Cargo.toml             # Package dependencies and binary definitions
├── Cargo.lock             # Locked dependency tree
└── src/
    ├── main.rs            # Application entrypoint, state setup, router, and graceful shutdown
    ├── config.rs          # Environment configuration loading & validation
    ├── error.rs           # Centralized application error types (`AppError`) and HTTP responses
    ├── models.rs          # Core domain data models and DTO structs
    ├── auth/              # OAuth handlers (Google/GitHub), JWT issuance & token refresh
    ├── collab/            # Real-time WebSocket room registry, broadcasting, & Yjs CRDT logic
    ├── connections/       # Encrypted connection profile CRUD handlers
    ├── db/                # MySQL pool setup, schema migrations, and SQL schema file
    ├── history/           # User query history CRUD handlers
    ├── middleware/        # Authentication middleware & header extractors
    └── queries/           # Saved SQL queries & folder synchronization handlers
```

---

## 🔌 API Routes Reference

### 1. Health Check
| Method | Endpoint | Description | Auth Required |
|---|---|---|---|
| `GET` | `/health` | Server health check (returns `ok`) | ❌ No |

### 2. Authentication (`/api/v1/auth`)
| Method | Endpoint | Description | Auth Required |
|---|---|---|---|
| `GET` | `/api/v1/auth/login/google` | Initiate Google OAuth2 login flow | ❌ No |
| `GET` | `/api/v1/auth/login/github` | Initiate GitHub OAuth2 login flow | ❌ No |
| `GET` | `/api/v1/auth/callback/google` | Handle Google OAuth2 callback & issue tokens | ❌ No |
| `GET` | `/api/v1/auth/callback/github` | Handle GitHub OAuth2 callback & issue tokens | ❌ No |
| `POST` | `/api/v1/auth/refresh` | Exchange a valid Refresh Token for a new Access Token | ❌ No |
| `POST` | `/api/v1/auth/logout` | Revoke active session / refresh token | 🔒 Yes |

### 3. Connection Profiles (`/api/v1/connections`)
| Method | Endpoint | Description | Auth Required |
|---|---|---|---|
| `GET` | `/api/v1/connections` | List all encrypted connection profiles for user | 🔒 Yes |
| `POST` | `/api/v1/connections` | Create a new encrypted connection profile | 🔒 Yes |
| `PUT` | `/api/v1/connections/{id}` | Update an existing encrypted connection profile | 🔒 Yes |
| `DELETE` | `/api/v1/connections/{id}` | Delete a connection profile | 🔒 Yes |

### 4. Query Execution History (`/api/v1/history`)
| Method | Endpoint | Description | Auth Required |
|---|---|---|---|
| `GET` | `/api/v1/history` | List execution history entries for user | 🔒 Yes |
| `POST` | `/api/v1/history` | Push a new query execution entry to history | 🔒 Yes |
| `DELETE` | `/api/v1/history` | Clear all execution history for user | 🔒 Yes |
| `DELETE` | `/api/v1/history/{id}` | Delete a single history entry | 🔒 Yes |

### 5. Saved Queries & Folders (`/api/v1/queries`)
| Method | Endpoint | Description | Auth Required |
|---|---|---|---|
| `GET` | `/api/v1/queries` | List saved queries & folder hierarchy | 🔒 Yes |
| `POST` | `/api/v1/queries` | Save a new query file | 🔒 Yes |
| `PUT` | `/api/v1/queries/{id}` | Update a saved query (checksum checked) | 🔒 Yes |
| `DELETE` | `/api/v1/queries/{id}` | Delete a saved query | 🔒 Yes |

### 6. Collaboration Rooms (`/api/v1/collab`)
| Method | Endpoint | Description | Auth Required |
|---|---|---|---|
| `GET` | `/api/v1/collab/rooms` | List collaborative rooms owned or joined by user | 🔒 Yes |
| `POST` | `/api/v1/collab/rooms` | Create a new collaboration room | 🔒 Yes |
| `DELETE` | `/api/v1/collab/rooms/{room_id}` | Delete a room (owner only) | 🔒 Yes |
| `GET` | `/api/v1/collab/rooms/{room_id}/members` | List members of a collaboration room | 🔒 Yes |
| `POST` | `/api/v1/collab/rooms/{room_id}/invite` | Invite a user to a room with role (`editor`/`viewer`) | 🔒 Yes |

### 7. Real-Time WebSocket (`/ws`)
| Protocol | Endpoint | Description | Auth Required |
|---|---|---|---|
| `WS` / `WSS` | `/ws/collab/{room_id}` | Real-time CRDT (Yjs) sync channel over WebSocket | 🔒 Yes |

---

## 🗄️ Database Schema Overview

`tabular-server` uses **MySQL** as its relational database. The schema is automatically initialized from [`src/db/schema.sql`](file:///Users/jayuda/Documents/PROJECT/TABULAR/tabular-server/src/db/schema.sql) upon startup.

Key tables:
- **`users`**: User identity records provisioned via OAuth (`google`, `github`).
- **`sessions`**: Active refresh tokens and device metadata.
- **`connections`**: Client-side encrypted database credentials stored as AES-256-GCM ciphertexts.
- **`query_history`**: Chronological log of queries executed by users.
- **`saved_queries`**: User SQL snippets with folder paths (`folder_path`) and SHA-256 checksums (`client_checksum`).
- **`collab_rooms`**: Collaboration room metadata and periodic binary Yjs document snapshots (`crdt_snapshot`).
- **`room_members`**: Room access permissions (`owner`, `editor`, `viewer`).
- **`oauth_states`**: Transient PKCE and state nonces for securing OAuth login flows.

---

## ⚙️ Environment Configuration

Create a `.env` file in `tabular-server/` based on [`.env.example`](file:///Users/jayuda/Documents/PROJECT/TABULAR/tabular-server/.env.example):

```env
# Database Connection
DATABASE_URL=mysql://root:password@localhost:3306/tabular_server

# Server Configuration
SERVER_PORT=8420
SERVER_BASE_URL=http://localhost:8420

# JWT Security
JWT_SECRET=change-this-to-a-long-random-secret-in-production
JWT_ACCESS_EXPIRY_MINUTES=60
JWT_REFRESH_EXPIRY_DAYS=30

# OAuth — Google
GOOGLE_CLIENT_ID=your-google-client-id.apps.googleusercontent.com
GOOGLE_CLIENT_SECRET=your-google-client-secret
GOOGLE_REDIRECT_URI=http://localhost:8420/api/v1/auth/callback/google

# OAuth — GitHub
GITHUB_CLIENT_ID=your-github-client-id
GITHUB_CLIENT_SECRET=your-github-client-secret
GITHUB_REDIRECT_URI=http://localhost:8420/api/v1/auth/callback/github

# CORS Settings (Comma-separated)
ALLOWED_ORIGINS=http://localhost:3000,http://localhost:8420
```

### 🔑 Cara Mendapatkan OAuth Credentials (Google & GitHub)

#### 1. Google OAuth2 (`GOOGLE_CLIENT_ID` & `GOOGLE_CLIENT_SECRET`)
1. Buka **[Google Cloud Console](https://console.cloud.google.com/)**.
2. Buat proyek baru (*New Project*) atau pilih proyek yang sudah ada.
3. Buka menu **APIs & Services** > **OAuth consent screen**:
   - Pilih User Type: **External** (atau **Internal** jika menggunakan Google Workspace organisasi).
   - Isi info dasar (App Name, User Support Email, Developer Contact Info).
   - Pada bagian **Scopes**, tambahkan scope `.../auth/userinfo.email` dan `.../auth/userinfo.profile`.
   - Simpan dan lanjutkan (*Save and Continue*). Jika status app masih *Testing*, tambahkan email Anda ke daftar **Test users**.
4. Buka menu **APIs & Services** > **Credentials**:
   - Klik **+ CREATE CREDENTIALS** > pilih **OAuth client ID**.
   - Pilih Application type: **Web application**.
   - Beri nama Client (contoh: `Tabular Server Local`).
   - Pada bagian **Authorized redirect URIs**, tambahkan:
     ```text
     http://localhost:8420/api/v1/auth/callback/google
     ```
   - Klik **Create**.
5. Salin **Client ID** (misal `xxx...apps.googleusercontent.com`) ke `GOOGLE_CLIENT_ID` dan **Client Secret** ke `GOOGLE_CLIENT_SECRET` di file `.env`.

#### 2. GitHub OAuth2 (`GITHUB_CLIENT_ID` & `GITHUB_CLIENT_SECRET`)
1. Login ke GitHub dan buka **[GitHub Developer Settings - OAuth Apps](https://github.com/settings/developers)**.
2. Klik tombol **New OAuth App**.
3. Isi formulir pendaftaran aplikasi:
   - **Application name**: `Tabular Server` (atau nama pilihan Anda)
   - **Homepage URL**: `http://localhost:8420`
   - **Authorization callback URL**:
     ```text
     http://localhost:8420/api/v1/auth/callback/github
     ```
4. Klik **Register application**.
5. Salin **Client ID** yang muncul dan tempel ke `GITHUB_CLIENT_ID` di file `.env`.
6. Klik tombol **Generate a new client secret**, lalu salin nilai secret tersebut ke `GITHUB_CLIENT_SECRET` di file `.env`.

---

## 🏁 Getting Started

### Prerequisites

- **Rust toolchain** (Rust 1.85+ recommended):
  ```bash
  rustup update stable
  ```
- **MySQL Server** (version 8.0+):
  Ensure a database named `tabular_server` (or matching your `DATABASE_URL`) exists:
  ```sql
  CREATE DATABASE tabular_server CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;
  ```

### Development Setup

1. **Clone & Navigate to `tabular-server`**:
   ```bash
   cd tabular-server
   ```

2. **Configure Environment**:
   ```bash
   cp .env.example .env
   # Edit .env with your MySQL credentials and OAuth keys
   ```

3. **Run the Server**:
   ```bash
   cargo run
   ```
   *The server will start, connect to MySQL, run missing migrations automatically, and listen on `http://localhost:8420`.*

4. **Verify Health Endpoint**:
   ```bash
   curl http://localhost:8420/health
   # Expected output: ok
   ```

### Building for Production

Compile an optimized release binary:
```bash
cargo build --release
```
The compiled executable will be available at `./target/release/tabular-server`.

### Running Tests

Run the test suite:
```bash
cargo test
```

---

## 🔒 Security Best Practices

1. **Zero-Knowledge Architecture**: Credentials stored in `connections` are encrypted client-side using `AES-256-GCM`. Server operators cannot decrypt database passwords stored in `tabular-server`.
2. **CSRF & PKCE Security**: All OAuth redirect flows use cryptographically generated state nonces stored in `oauth_states` with short expiration times.
3. **Prepared SQL Statements**: Database interactions use SQLx compile-time checked queries and parameterized bindings to eliminate SQL injection risks.
4. **Token Isolation**: Refresh tokens are stored as hashed strings in the database and strictly checked against expiration dates.

---

## 📄 License

This project is part of the **Tabular** workspace. Licensed under the [MIT License](../LICENSE).
