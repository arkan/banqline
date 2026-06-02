# Banqline CLI Workflows Reference

## Table of Contents

- Mental model
- Configuration and local state
- Command map
- Setup and authorization
- Synchronization
- Accounts, balances, and transactions
- Reports and forecasts
- Tagging
- Alerts
- Multi-bank and account selection
- Output formats and automation
- Troubleshooting playbook
- Recommended user-facing guidance

## Mental Model

Banqline is local-first. Separate these concerns:

- Configuration: local YAML config containing Enable Banking app settings, tag rules, and alert rules.
- Session store: local `session.json` created by `bank connect`; proves authorization and stores bank/account session metadata.
- SQLite cache: local `data.db` filled by `sync`; read commands use this cache.

Important consequences:

- `bank connect` does not by itself guarantee `account list`, `tx list`, or reports have data.
- `bank status` can show valid sessions and account counts even when the SQLite cache is empty.
- `account list` reads cached accounts.
- `balance list` reads cached balances.
- `tx list`, `report summary`, tags, and alerts read cached transactions.
- `sync` talks to the bank API and updates the cache.

## Configuration and Local State

Default paths come from the platform config directory plus `banqline`. On macOS this is commonly:

```text
~/Library/Application Support/banqline/config.yaml
~/Library/Application Support/banqline/session.json
~/Library/Application Support/banqline/data.db
```

On Linux this is commonly:

```text
~/.config/banqline/config.yaml
~/.config/banqline/session.json
~/.config/banqline/data.db
```

Minimal config:

```yaml
application_id: "your-enable-banking-application-id"
key_path: "private.key"
redirect_url: "http://localhost:19876/callback"
callback_port: 19876
default_bank: "YOUR_BANK_NAME"
log_level: "info"
```

`key_path` is resolved by file name inside the Banqline app directory. For example `private.key`, `./private.key`, and `/old/path/private.key` all resolve to the key file inside the app directory.

Local SQLite tables:

- `accounts`: filled by `sync accounts` or `sync all`.
- `balances`: filled by `sync accounts`, `sync balances`, or `sync all`.
- `transactions`: filled by `sync tx` or `sync all`.
- `sync_meta`: last sync timestamps by account and data type.

Never expose private keys, JWTs, session JSON, or full local database contents unless the user explicitly requests it and understands the sensitivity.

## Command Map

Top-level:

```bash
banqline version
banqline doctor
banqline bank <command>
banqline sync [command]
banqline account <command>
banqline balance <command>
banqline tx <command>
banqline report <command>
banqline alert <command>
banqline tui
```

Global output flag:

```bash
banqline --format table <command>
banqline --format json <command>
banqline --format csv <command>
```

Prefer placing `--format` before the subcommand.

Bank commands:

```bash
banqline bank list --country FR
banqline bank list --country FR --filter "Credit"
banqline bank connect --country FR --bank "<exact bank name>"
banqline bank status
```

Sync commands:

```bash
banqline sync
banqline sync all --bank "<bank>" --from YYYY-MM-DD --to YYYY-MM-DD
banqline sync accounts --bank "<bank>"
banqline sync balances --bank "<bank>" --account "<uid-or-iban-suffix-or-alias>"
banqline sync tx --bank "<bank>" --account "<uid-or-iban-suffix-or-alias>" --from YYYY-MM-DD --to YYYY-MM-DD
```

`banqline sync` with no target behaves like `sync all` with default flags.

Read commands:

```bash
banqline account list --bank "<bank>"
banqline balance list --bank "<bank>" --account "<uid-or-iban-suffix-or-alias>"
banqline tx list --bank "<bank>" --account "<account>" --from YYYY-MM-DD --to YYYY-MM-DD --limit 100
banqline tx list --category groceries --direction DBIT
```

Alias commands:

```bash
banqline account alias set --alias checking --uid "<account uid>"
banqline account alias get checking
banqline account alias remove checking
banqline account alias list
```

Report commands:

```bash
banqline report summary --bank "<bank>" --period month
banqline report summary --bank "<bank>" --period week --compare
banqline report summary --bank "<bank>" --from YYYY-MM-DD --to YYYY-MM-DD
banqline report forecast --bank "<bank>"
banqline report forecast --all
banqline report forecast --all --detail
```

Tag commands:

```bash
banqline tx tag preview
banqline tx tag apply
banqline tx tag override --id "<transaction id>" --category groceries
banqline tx tag interactive
```

`tx tag interactive` currently reports that interactive tagging is not implemented.

Alert commands:

```bash
banqline alert add --type transaction --amount-gte 100 --direction DBIT --merchant-contains "AMAZON"
banqline alert add --type category --category groceries --threshold 500 --period month --bank "<bank>"
banqline alert list
banqline alert check
banqline alert check --json
banqline alert remove "<alert name>"
```

## Setup and Authorization

Recommended first-run sequence:

```bash
banqline doctor
banqline bank list --country FR --filter "<bank keyword>"
banqline bank connect --country FR --bank "<exact bank name from bank list>"
banqline bank status
banqline sync all --bank "<exact bank name>" --from YYYY-MM-DD
```

Interpretation:

- `doctor` checks config, app ID, key, callback port, session, and database path.
- `bank list` discovers the exact bank name expected by Enable Banking.
- `bank connect` opens a browser, receives the authorization callback, creates a session, polls accounts, and writes the session store.
- `bank status` verifies local sessions and expiration.
- `sync all` is the first command that populates the operational cache.

If `bank connect` succeeds but read commands are empty, the next action is usually sync, not reconnect.

## Synchronization

Choose the smallest sync that populates the data needed:

- Use `sync accounts` after a fresh connection when only account list and balances are needed.
- Use `sync balances` to refresh balances without fetching transactions.
- Use `sync tx --from YYYY-MM-DD` to populate transaction history for lists, summaries, tags, alerts, and forecasts with pending movements.
- Use `sync all --from YYYY-MM-DD` for first full population or broad refresh.

Date range guidance:

- Use an explicit `--from` for useful history. Without history, reports can be empty.
- Use `--to` for bounded exports or debugging API behavior.
- For monthly summaries, syncing from the first day of the current month is enough.
- For comparisons, sync enough history for both compared periods.

Examples:

```bash
banqline sync accounts --bank "<bank>"
banqline sync tx --bank "<bank>" --from 2026-01-01
banqline sync all --bank "<bank>" --from 2026-01-01
banqline sync tx --account checking --from 2026-05-01 --to 2026-05-31
```

## Accounts, Balances, and Transactions

Accounts:

- `account list` reads cached rows from `accounts`.
- It can be empty even when `bank status` shows accounts in the session.
- Fix empty account list with `sync accounts` or `sync all`.

Balances:

- `balance list` reads cached balances.
- It defaults to the single session account if there is only one; otherwise specify `--account`.
- Refresh with `sync balances`, `sync accounts`, or `sync all`.

Transactions:

- `tx list` reads cached transactions.
- Use `--from`, `--to`, `--limit`, `--category`, and `--direction` to narrow results.
- Common direction values come from Open Banking transaction indicators, such as `DBIT` for debits and `CRDT` for credits.
- If output is empty, verify sync history and filters.

Useful sequence:

```bash
banqline sync tx --bank "<bank>" --from 2026-01-01
banqline tx list --bank "<bank>" --from 2026-01-01 --limit 20
banqline --format json tx list --bank "<bank>" --from 2026-01-01 --limit 20
```

## Reports and Forecasts

Summary:

- `report summary` reads cached booked transactions only.
- It filters transactions with status `BOOK`.
- Default period is `month`.
- Default date range is from the start of the current period to today.
- `--compare` expands the date range to include the previous period.
- If it prints only headers, likely causes are no synced transactions, no `BOOK` transactions, or a date range with no data.

Examples:

```bash
banqline sync tx --from 2026-01-01
banqline report summary --period month
banqline report summary --period month --compare
banqline report summary --from 2026-01-01 --to 2026-12-31
banqline --format json report summary --bank "<bank>" --period week
```

Forecast:

- `report forecast` uses cached balances plus cached pending transactions.
- Pending transactions are transactions with status `PDNG`.
- If pending is zero, either there are no pending transactions or transactions have not been synced recently.
- Use `--detail` for per-account pending transaction details.
- Use `--all` for all valid sessions.

Examples:

```bash
banqline sync all --from 2026-01-01
banqline report forecast --all
banqline report forecast --bank "<bank>" --detail
```

## Tagging

Tag rules live in `config.yaml` under `tag_rules`.

Example:

```yaml
tag_rules:
  groceries:
    - carrefour
    - auchan
  transport:
    - sncf
    - ratp
```

Workflow:

```bash
banqline sync tx --from 2026-01-01
banqline tx tag preview
banqline tx tag apply
banqline tx list --category groceries --from 2026-01-01
```

Rules:

- `tx tag preview` shows changes without writing.
- `tx tag apply` updates non-manual transactions.
- `tx tag override --id ... --category ...` sets a manual category.
- Manual overrides are preserved by later `tx tag apply`.
- Tagging requires transactions in the cache.

## Alerts

Alert rules live in `config.yaml` under `alert_rules`; `alert add` writes them.

Transaction alert example:

```bash
banqline alert add \
  --name large-card-debit \
  --type transaction \
  --amount-gte 100 \
  --direction DBIT
```

Merchant alert example:

```bash
banqline alert add \
  --type transaction \
  --merchant-contains "AMAZON" \
  --direction DBIT
```

Category alert example:

```bash
banqline alert add \
  --type category \
  --category groceries \
  --threshold 500 \
  --period month \
  --bank "<bank>"
```

Workflow:

```bash
banqline sync tx --from 2026-01-01
banqline tx tag apply
banqline alert list
banqline alert check
```

Notes:

- Category alerts require the category to exist in `tag_rules`.
- Alerts read cached transactions.
- `alert check` exits with a status code from the alert engine; do not treat a non-zero exit as a CLI crash without reading the output.
- If sessions are expired, alert output may ask the user to run `bank connect` again.

## Multi-Bank and Account Selection

When there is exactly one valid session, commands can often omit `--bank`.

When multiple banks are configured:

- Use `--bank "<exact bank name>"`.
- Use `bank status` to list known bank names.
- Use `sync --bank` to avoid accidentally refreshing every bank.

Account selectors can be:

- Account UID.
- Full IBAN.
- IBAN suffix when long enough to identify the account.
- Alias configured with `account alias set`.

Recommended alias setup:

```bash
banqline account list --bank "<bank>"
banqline account alias set --alias checking --uid "<account uid>"
banqline balance list --account checking
banqline tx list --account checking --from 2026-01-01
```

## Output Formats and Automation

Use JSON for scripts and diagnostics:

```bash
banqline --format json doctor
banqline --format json bank status
banqline --format json tx list --from 2026-01-01 --limit 100
banqline --format json report summary --period month
```

Use CSV for exports where supported:

```bash
banqline --format csv tx list --from 2026-01-01 --limit 1000
banqline --format csv doctor
```

Keep `--format` before the subcommand to match the documented CLI shape.

## Troubleshooting Playbook

### `No accounts found for <bank>`

Likely cause: session exists, but `accounts` table is empty.

Check:

```bash
banqline bank status
banqline account list --bank "<bank>"
```

Fix:

```bash
banqline sync accounts --bank "<bank>"
```

### `report summary` prints only headers

Likely causes:

- No transactions synced.
- Transactions are outside the summary date range.
- Synced transactions are not status `BOOK`.
- Wrong bank filter.

Check:

```bash
banqline tx list --bank "<bank>" --from 2026-01-01 --limit 5
banqline report summary --bank "<bank>" --from 2026-01-01 --to 2026-12-31
```

Fix:

```bash
banqline sync tx --bank "<bank>" --from 2026-01-01
```

### `balance list` is empty or stale

Fix:

```bash
banqline sync balances --bank "<bank>"
```

or:

```bash
banqline sync accounts --bank "<bank>"
```

### `tx list` is empty

Check filters first:

```bash
banqline tx list --bank "<bank>" --from 2026-01-01 --limit 20
```

Then sync:

```bash
banqline sync tx --bank "<bank>" --from 2026-01-01
```

### `multiple banks configured; specify --bank`

Use:

```bash
banqline bank status
banqline <command> --bank "<exact bank name>"
```

### `multiple accounts; specify --account`

Use:

```bash
banqline account list --bank "<bank>"
banqline <command> --account "<uid-or-iban-suffix-or-alias>"
```

### Session expired

Use:

```bash
banqline bank connect --country <ISO2> --bank "<bank>"
banqline bank status
```

Then sync again if cache freshness matters.

### OAuth callback or browser issues

Check:

```bash
banqline doctor
```

Verify:

- `redirect_url` matches the configured callback URL in Enable Banking.
- `callback_port` is free.
- `application_id` and private key are configured.
- The private key file is in the Banqline app directory or `key_path` resolves to the expected file name.

### API decode errors

If the CLI reports `decode response`, suspect an API response shape mismatch. Capture only the error chain by default. Avoid dumping sensitive API payloads. Reproduce with the narrow command, then inspect or patch the response type in `src/client/types.rs` and add a serde test.

## Recommended User-Facing Guidance

Prefer direct commands and short explanations:

- "Your session exists, but your local cache is empty. Run `banqline sync accounts ...`."
- "Your report is empty because no transactions are cached. Run `banqline sync tx --from ...`."
- "Use `bank status` for sessions and `account list` for cached accounts; they answer different questions."
- "Use `--bank` once you have multiple sessions."
- "Use `--format json` before the command for machine-readable output."

When suggesting commands, preserve exact bank names in quotes and include `--from` for transaction/report workflows unless the user explicitly wants only the default current period.
