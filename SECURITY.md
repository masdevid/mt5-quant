# Security Policy

This document describes how to report security issues in **mt5-quant**, and the
secure-development practices the project follows. It backs the OpenSSF Best
Practices criteria `vulnerability_report_process`, `know_secure_design`, and
`know_common_errors`, and the signed-release process described under
[Signing](#signing-of-release-artifacts) backs `delivery_unsigned`.

mt5-quant is a Rust MCP server that drives MetaTrader 5 (under Wine/CrossOver)
to compile, backtest, and analyze trading Expert Advisors. It runs **locally**;
it never exposes a network listener and never transmits user data off the
machine except for the opt-in version self-check (HTTPS to GitHub). The security
notes below focus on the real risks in that model: untrusted input from MT5
report files, the user-supplied Wine/MT5 configuration, and the build/publish
pipeline.

## Vulnerability reporting process

We encourage responsible disclosure. If you discover a vulnerability, please
report it through **one** of the following channels (in order of preference):

1. **GitHub Security Advisories (recommended).** Open a private security
   advisory at <https://github.com/masdevid/mt5-quant/security/advisories/new>.
   This keeps the details confidential while we prepare a fix.
2. **Public GitHub Issue.** If you prefer, open a normal issue at
   <https://github.com/masdevid/mt5-quant/issues>. Public reports are accepted;
   please avoid posting working exploits in the open issue body — link to a
   private gist or email the maintainer instead.

### What to include

A useful report contains:

- A clear description of the vulnerability and its impact.
- The affected version(s) (`mt5-quant --version` or the `version` field in
  `Cargo.toml`).
- Steps to reproduce, or a proof-of-concept input (e.g. a crafted MT5 report,
  `.set` file, or tool argument).
- Any relevant logs, crashes, or sanitizer/fuzzer output.

### Expected handling timeline

- **Acknowledgement:** within 7 days of a complete report.
- **Triage & severity assessment:** within 14 days.
- **Fix or mitigation plan:** timeline depends on severity; critical issues are
  addressed as soon as a patch is ready, and a coordinated disclosure date is
  agreed with the reporter when a private advisory is used.
- **Disclosure:** once a fixed release is published, the advisory (if used) is
  made public and the fix is credited to the reporter unless they request
  anonymity.

There is no formal contractual SLA; reports are handled on a best-effort,
case-by-case basis through the public channels above.

## Secure development practices (`know_secure_design`)

mt5-quant follows these practices to keep the code trustworthy:

- **Input validation at every tool boundary.** All 89 MCP tool handlers in
  `src/tools/handlers/` parse and validate their arguments at the entry point
  (using helpers such as `required_str`, `resolve_report`, and
  `prepare_analysis`). Untrusted data — tool arguments, the filesystem, and MT5
  output — is never trusted inside core logic.
- **No hardcoded secrets.** The source tree contains no embedded credentials,
  API tokens, or private keys. The only secrets are user-provided: a local
  `config/mt5-quant.yaml` (gitignored) holding the Wine/MT5 paths, and an
  optional release PAT used only in CI. Neither is persisted by the project
  itself.
- **Safe parsing of untrusted MT5 reports.** Backtest/optimization reports come
  from MetaTrader 5 as either HTML or SpreadsheetML XML (`.htm.xml`, Build 48+).
  Parsers in `src/analytics/extract.rs` and `src/optimization/parser.rs` use
  explicit, bounded parsing (no `eval`, no regex-backed HTML/script execution)
  and fail fast on malformed input. These parsers are additionally exercised by
  the cargo-fuzz harness in `fuzz/` (see `dynamic_analysis*` criteria).
- **Least privilege at runtime.** The architecture enforces **a single MT5
  instance** and never runs two backtests in parallel on the same Wine prefix.
  When no display is available the pipeline falls back to headless/Xvfb mode; the
  only external process launched is the user-configured Wine/MT5 binary.
- **Dependency auditing in CI.** Every push and pull request runs
  `cargo-deny` (license + advisory + source checks) and the GitHub
  `dependency-review` action. Vulnerable or non-compliant dependencies block the
  build via the `protect-master` ruleset's required status checks.
- **Fuzzing of parsers.** `fuzz/` contains libFuzzer targets that feed arbitrary
  bytes into the report/metrics parsers with debug assertions and integer-
  overflow traps enabled, catching panics and invariant violations.
- **Isolation of the Wine/MT5 sandbox.** MT5 runs under the user's Wine prefix,
  which is isolated from the host shell environment. The server does not grant
  MT5 any elevated privileges.
- **Static analysis.** Clippy runs with `-D warnings` (warnings fail the build)
  and GitHub CodeQL (Rust) scans every push/PR.

## Common implementation errors we avoid (`know_common_errors`)

The codebase is deliberately written to avoid well-known Rust/systems pitfalls:

- **Integer overflow.** Rust's release builds disable overflow checks by
  default, so arithmetic on deal volumes, prices, and cumulative PnL uses
  `checked_*`/`saturating_*` operations or is validated at parse time. The
  fuzz harness re-enables `debug-assertions` so overflow traps fire during
  testing.
- **Path traversal in `.set` file handling.** `.set` files (UTF-16LE parameter
  files for EAs) are read/written through helpers in
  `src/tools/handlers/setfiles.rs`. Paths are resolved against the configured
  tester profiles directory and normalized; user-supplied filenames are never
  interpolated into shell commands and are confined to that directory.
- **UTF-16LE encoding pitfalls.** MetaTrader strips `||Y` flags from UTF-8 `.set`
  files, so all writes use UTF-16LE with an explicit BOM and `chmod 444`. The
  helpers never assume UTF-8, avoiding silent mojibake / flag loss.
- **Avoiding `unsafe`.** The crate uses no `unsafe` blocks in application logic;
  all FFI with Wine/MT5 is performed via the `std::process` API and the system
  `curl`/`gpg` binaries, not raw pointer manipulation.
- **Proper error propagation.** Errors are returned as typed `Result`s and
  propagated with `?`; there are no swallowed errors or empty `catch`-and-ignore
  paths. Invalid states halt fast with descriptive messages (Fail-Fast
  philosophy) rather than being silently patched.
- **No injection via shell.** Wine/MT5 is launched with an argument vector (not
  a constructed shell string), eliminating command-injection from config values.

## Signing of release artifacts

To satisfy `delivery_unsigned`, **release artifacts are GPG-signed**:

- Each GitHub Release publishes the macOS (`mcp-mt5-quant-macos-arm64.tar.gz`)
  and Linux (`mcp-mt5-quant-linux-x64.tar.gz`) binaries **plus** their detached
  ASCII-armored signatures (`*.tar.gz.asc`), and the project's public key at
  **`docs/signing-key.asc`**.
- The signing key is an RSA-4096 key generated for the `mt5-quant releases`
  identity. The public half is committed to the repository; the private half is
  held only in CI secrets (`GPG_PRIVATE_KEY`, `GPG_PASSPHRASE`) and is never
  committed.
- Signing is performed in CI (`release.yml`) with
  `gpg --detach-sign --armor` and is **guarded** so the release still succeeds
  if the signing secrets are not configured — it simply omits the signatures in
  that case.

### Verifying a downloaded release

```bash
# 1. Import the project public key
gpg --import docs/signing-key.asc

# 2. Verify a release artifact
gpg --verify mcp-mt5-quant-linux-x64.tar.gz.asc \
            mcp-mt5-quant-linux-x64.tar.gz

# A good signature from "mt5-quant releases <releases@mt5-quant.local>"
# confirms the artifact was produced by the project's release pipeline.
```

## Supported versions

Security fixes are applied to the latest released version on the `master`
branch. Older releases are not back-ported; users should upgrade to the current
release to receive fixes.
