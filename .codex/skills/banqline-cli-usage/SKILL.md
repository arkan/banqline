---
name: banqline-cli-usage
description: Operate and troubleshoot the Banqline terminal banking CLI. Use when Codex needs to recommend Banqline commands, diagnose empty output or stale local cache, explain authentication versus synchronization, work with accounts, balances, transactions, summaries, forecasts, tags, alerts, JSON/CSV output, multi-bank sessions, or help a user get value from Banqline without misusing the CLI.
---

# Banqline CLI Usage

## Core Model

Treat Banqline as a local-first CLI with two separate phases:

1. `bank connect` authorizes a bank and stores a session.
2. `sync ...` fills the local SQLite cache used by read commands.

Do not assume a successful bank connection means accounts, balances, transactions, reports, tags, or alerts have data. Most read commands use the local cache only.

## First Checks

When diagnosing behavior, inspect state in this order:

```bash
banqline doctor
banqline bank status
banqline account list --bank "<bank name>"
banqline tx list --bank "<bank name>" --from YYYY-MM-DD --limit 5
```

Use `--format json` before the command when machine-readable output helps:

```bash
banqline --format json bank status
banqline --format json tx list --bank "<bank name>" --from YYYY-MM-DD
```

Avoid printing secrets, private keys, full session files, or unnecessary full IBANs. Prefer command output, row counts, or masked identifiers.

## Workflow Decision Tree

- Need first setup or broken configuration: use `doctor`, then check `config.yaml`, `application_id`, `key_path`, `redirect_url`, and callback port.
- Need supported banks: use `bank list --country <ISO2>` and optional `--filter`.
- Need OAuth authorization: use `bank connect --country <ISO2> --bank "<exact bank name>"`.
- Need to verify the connection: use `bank status`. It reports sessions and account count from the session store.
- `account list` says no accounts: run `sync accounts` or `sync all`.
- `balance list` is empty or stale: run `sync balances`, `sync accounts`, or `sync all`.
- `tx list` or `report summary` is empty: run `sync tx --from YYYY-MM-DD` or `sync all --from YYYY-MM-DD`.
- `report forecast` has no pending effect: synchronize transactions; forecast uses balances plus pending transactions.
- Tags do not change anything: ensure transactions are synced and `tag_rules` exist, then run `tx tag preview` before `tx tag apply`.
- Alerts do not trigger: ensure transactions are synced, tags are applied when category alerts are used, and run `alert check`.

## High-Value Command Sequences

Connect and populate a bank:

```bash
banqline doctor
banqline bank list --country FR --filter "Credit"
banqline bank connect --country FR --bank "<exact bank name>"
banqline bank status
banqline sync all --bank "<exact bank name>" --from 2026-01-01
banqline account list --bank "<exact bank name>"
banqline report summary --bank "<exact bank name>" --from 2026-01-01
```

Daily refresh:

```bash
banqline sync all --from 2026-01-01
banqline report forecast --all
banqline alert check
```

Transaction review:

```bash
banqline sync tx --bank "<bank name>" --from 2026-01-01
banqline tx list --bank "<bank name>" --from 2026-01-01 --limit 100
banqline report summary --bank "<bank name>" --period month --compare
```

## Detailed Reference

Read `references/cli-workflows.md` when you need complete command syntax, local state details, cache semantics, troubleshooting cases, config examples, tag and alert workflows, or multi-bank/account guidance.
