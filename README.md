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
- output either human-readable tables or JSON.

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
~/.banqline/config.yaml
```

Minimal example:

```yaml
application_id: "your-enable-banking-application-id"
key_path: "~/.banqline/private.key"
redirect_url: "http://localhost:19876/callback"
callback_port: 19876
default_bank: "YOUR_BANK_NAME"
log_level: "info"
```

You need an Enable Banking application and a PEM-encoded RSA private key configured in the Enable Banking console. Keep private keys outside Git.

## Usage

```bash
# Show version
banqline version

# List banks for a country
banqline banks --country FR

# Authenticate with a bank
banqline auth --country FR --bank BANK_NAME

# List accounts
banqline accounts

# Display balances
banqline balances

# Fetch recent transactions
banqline transactions --from 2026-01-01 --limit 100

# Machine-readable output
banqline --json transactions --from 2026-01-01 --limit 100

# Spending summary
banqline summary --period month --compare

# Forecast
banqline forecast --all --detail

# Interactive TUI
banqline tui
```

Run `banqline <command> --help` for command-specific options.

## Local data and security

Banqline stores sensitive runtime data locally, under `~/.banqline` by default. This may include:

- `config.yaml` — application configuration;
- `session.json` — bank sessions/tokens;
- local SQLite data/cache files;
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
