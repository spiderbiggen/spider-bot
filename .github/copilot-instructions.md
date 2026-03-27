# Copilot Instructions

## Commands

```bash
# Build
cargo build

# Test
cargo test --no-fail-fast           # full suite
cargo test <test_name>              # single test, e.g. `cargo test format_balance`

# Lint (CI requires a clean run)
cargo clippy --all-targets --workspace -- -D warnings

# Format
cargo fmt           # apply
cargo fmt --check   # check only (used in CI)
```

> Clippy runs with `-D warnings` — all warnings are treated as errors. Keep it clean locally before committing.

## Architecture

This is a Cargo workspace with one binary crate (`src/`) and five library crates:

| Crate | Purpose |
|---|---|
| `src/` | Discord bot binary — poise/serenity framework |
| `db/` | PostgreSQL access layer via SQLx |
| `domain/` | Shared data types (no logic) |
| `klipy/` | HTTP client for the Klipy GIF API |
| `otaku/` | gRPC subscriber for anime episode notifications |
| `proto/` | Protobuf definitions, compiled at build time via tonic-prost-build |

### Main binary layout (`src/`)

- **`context.rs`** — trait extensions (`GifContextExt`, `KlipyExt`, etc.) that expose bot state from poise's `Context`. Commands always access shared state through these traits, never directly.
- **`cache.rs`** — GIF cache with a single async writer task and lock-free reads. Writes go through an `mpsc::Sender` (fire-and-forget); reads go directly to the `Arc<DashMap>`. Never hold the write path in an async critical section.
- **`commands/gifs.rs`** and submodules — slash commands; each command retrieves GIFs from the cache or falls back to the Klipy API.
- **`background_tasks.rs`** — GIF cache refresh (every 6 hours, aligned to the previous period boundary) + periodic cache trim + anime embed sender.

### Data flow for anime notifications

`otaku::subscribe` (gRPC stream) → `mpsc::channel` → `DiscordApi::embed_sender` → Discord channels/DMs

## Key Conventions

### Logging — always use `tracing`, never `println!`/`eprintln!`

```rust
tracing::info!("Connected");
tracing::error!("Failed: {err}");
```

Structured fields go before the message string:

```rust
tracing::error!(query, "Error fetching gifs for: {error}");
```

### Clippy suppressions — use `#[expect]`, not `#[allow]`

```rust
// ✅ correct — will fail if the lint is ever resolved
#[expect(clippy::cast_possible_wrap)]

// ❌ wrong — silently stays even if the code changes
#[allow(clippy::cast_possible_wrap)]
```

### Error types

- `thiserror` for all library/command error enums.
- `anyhow` only in `main()`.
- `CommandError` (in `src/commands.rs`) wraps `GifError`, `serenity::Error`, and `db::Error` via `#[from]`.

### Guild-only commands

Commands that need a guild context carry `guild_only` and access `guild_id` with a `let-else`:

```rust
#[poise::command(slash_command, guild_only)]
pub async fn my_command(ctx: Context<'_, '_>) -> Result<(), CommandError> {
    let Some(guild_id) = ctx.guild_id() else {
        return Ok(());
    };
    let guild_id = guild_id.get(); // u64
    // ...
}
```

### SQLx queries

SQL lives in files under `db/queries/`, not inline. Use the compile-time macros:

```rust
sqlx::query_file!("queries/balance/get_user_balance.sql", guild_id as i64, user_id as i64)
sqlx::query_file_scalar!("queries/balance/add_user_balance.sql", ...)
```

Discord/PostgreSQL IDs are stored as `i64` in the DB (Postgres has no unsigned type); cast with `#[expect(clippy::cast_possible_wrap)]`.

### Protobuf

Editing `proto/protos/api.v2.proto` triggers automatic recompilation via the build script — no manual step needed.

### Docker targets

CI builds static musl binaries (`x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`) deployed into a `distroless/static-debian12` image. `Local.Dockerfile` uses `distroless/cc-debian12` (glibc, for development). `Cross.toml` configures the same musl targets for local use with the `cross` tool.
