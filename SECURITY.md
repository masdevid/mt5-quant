# Security Policy

## Supported Versions

Only the latest minor release line receives security fixes. Older versions should upgrade.

| Version | Supported          |
| ------- | ------------------ |
| 1.34.x  | :white_check_mark: |
| < 1.34  | :x:                |

## Reporting a Vulnerability

**Please do not open public GitHub issues for security matters.**

Preferred: use GitHub [Private Vulnerability Reporting](https://github.com/masdevid/mt5-quant/security/advisories/new) — **Security tab → Report a vulnerability**. This keeps the report confidential while we work on a fix.

Please include as much of the following as you can:

- Affected version (and commit SHA, if relevant)
- Steps to reproduce or a proof of concept
- Expected vs. actual behavior
- Any known mitigations

## Response Commitment

- **Acknowledgement:** within **72 hours** of your report
- **Fix target:** vulnerabilities rated **critical/high** are targeted for a patch release within **14 days**
- We will keep you informed of progress and credit you in the advisory unless you prefer otherwise

## Scope Notes

MT5-Quant drives Wine and MetaTrader 5 locally — it launches external processes with paths, arguments, and configuration sourced from its YAML config file, `.set` parameter files, and MCP tool arguments. Reports concerning path traversal, config injection, or unsafe command construction through these surfaces are very much in scope and welcome.

Out of scope: vulnerabilities in Wine, MetaTrader 5 itself, or broker infrastructure — please report those upstream to the respective vendors.
