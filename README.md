# Banqline

**Banqline** is a terminal-first personal banking CLI and TUI built on top of the [Enable Banking](https://enablebanking.com/) Open Banking API. It is designed for power users who want to inspect accounts, balances, transactions, forecasts, summaries, tags, and alerts without leaving the command line.

The repository is private while the project is being shaped, but the goal is to make it suitable for open source release once the API surface, security model, and documentation are stable.

## Why Banqline?

Most banking apps are optimized for occasional mobile usage. Banqline is for people who live in the terminal and want banking data that is:

- **scriptable** — JSON output for automation, exports, and LLM workflows;
- **fast to inspect** — tables for humans, filters for transactions;
- **local-first** — configuration, sessions, aliases, cached data, tags, and alerts live on your machine;
- **privacy-conscious** — no hosted backend, no shared service, no secrets committed to the repository.

## Features

Current and planned capabilities include:

- list supported banks by country;
- authenticate with a bank through Enable Banking OAuth;
- list accounts and account aliases;
- display balances;
- fetch and filter transactions;
- compute spending summaries by period and category;
- forecast upcoming balance changes from known operations;
- tag transactions using configurable rules;
- define and check local balance/transaction alerts;
- launch an interactive terminal UI dashboard;
- output human-readable tables, JSON, or CSV via `--format`.

## Installation

### From source

```bash
git clone https://github.com/arkan/banqline.git
cd banqline
cargo build --release
```

The binary is produced at:

```bash
./target/release/banqline
```

You can also install it locally:

```bash
cargo install --path . --bin banqline --force
```

Or use the Makefile:

```bash
make build
make install
```

## Configuration

Banqline uses a YAML configuration file, by default:

```text
~/.config/banqline/config.yaml
```

Minimal example:

```yaml
application_id: "your-enable-banking-application-id"
key_path: "private.key"
redirect_url: "http://localhost:19876/callback"
callback_port: 19876
default_bank: "YOUR_BANK_NAME"
log_level: "info"
```

`key_path` is resolved inside `~/.config/banqline`: `private.key`, `./private.key`, or `/some/old/path/private.key` all resolve to `~/.config/banqline/private.key`.

You need an Enable Banking application and a PEM-encoded RSA private key configured in the Enable Banking console. Keep private keys outside Git.

## Usage

```bash
# Show version
banqline version

# Diagnose local setup before connecting a bank
banqline doctor
banqline --format json doctor

# List banks for a country
banqline bank list --country FR

# Authenticate with a bank
banqline bank connect --country FR --bank BANK_NAME

# Check local bank sessions
banqline bank status

# Sync local cache from bank APIs
banqline sync
banqline sync tx --from 2026-01-01
banqline sync balances

# List accounts from local cache
banqline account list

# Display cached balances
banqline balance list

# Read cached transactions
banqline tx list --from 2026-01-01 --limit 100

# Machine-readable output
banqline --format json tx list --from 2026-01-01 --limit 100
banqline --format csv tx list --from 2026-01-01 --limit 100

# Spending summary
banqline report summary --period month --compare

# Forecast
banqline report forecast --all --detail

# Interactive TUI (? opens the shortcut help)
banqline tui
```

Run `banqline <command> --help` for command-specific options.

`sync` talks to the bank API and updates the local cache. `list`, `report`, and `alert check` read from the local cache only.

## Local data and security

Banqline stores all local state under `~/.config/banqline` by default. This may include:

- `~/.config/banqline/config.yaml` — application configuration;
- `~/.config/banqline/session.json` — bank sessions/tokens;
- `~/.config/banqline/data.db` — local SQLite data/cache;
- the PEM private key referenced by `key_path`;
- transaction tags and alert rules.

Security expectations:

- config and session files should be written with restricted permissions;
- private keys must never be committed;
- `.env`, `*.pem`, `*.key`, SQLite databases, and local state are ignored by Git;
- there is no hosted Banqline backend.

## Development

```bash
# Format
cargo fmt

# Run tests
cargo test

# Build release binary
cargo build --release --bin banqline

# Clean generated artifacts
make clean
```

## Project status

Banqline is early-stage software. The repository is private for now and will be cleaned up further before a future open source release. Expect CLI details, storage format, and APIs to evolve.

## License

MIT — see [`LICENSE`](LICENSE).
