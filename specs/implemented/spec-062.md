# Spec-062: CLI (clap) entry point & exit codes

> Plan reference: §1 ("CLI (clap)"), §3 (`crates/rglint/src/{main,cli,exit}.rs`)

## Goal

Implement the `rglint` binary CLI using `clap` derive. Wires together config
discovery (spec-054/055), project resolution (spec-007), the engine
(spec-011), reporters (specs 057-060), `--fix` (spec-061), and exit codes.

## Scope

**In scope:**

- `cli.rs` — clap derive:
  ```
  rglint [OPTIONS] [PATH]...
    --config <FILE>          override config discovery
    --format <pretty|json|sarif|github>
    --fix                    apply suggestions in place
    --fix-dry-run            print fixes, don't write
    --no-color               disable colored output
    --quiet                  suppress summary
    --max-warnings <N>       exit non-zero if warning count exceeds N
    --rule <ruleId>          enable a rule (overrides config)
    --rulesdir <DIR>         (future; stub)
    --init                   write a default .rglintrc
    -h, --help
    -V, --version
  ```
- `main.rs` — orchestrate: discover config → resolve projects → run engine →
  render → set exit code.
- `exit.rs` — `ExitCode` mapping: `0` clean; `1` lint errors; `2`
  config/usage error; `3` internal error (panic-equivalent).
- `--init`: write a `.rglintrc.toml` with the default preset (spec-063)
  commented out.
- `--max-warnings`: exit `1` if `errors > 0` OR `warnings > max_warnings`.
- Progress: per-file progress on stderr when not `--quiet` and stderr is a TTY.

**Out of scope:**

- LSP server (stretch).
- Watch mode (stretch).

## Dependencies

- specs 054, 055 (config), 007 (projects), 011 (engine), 057-060 (reporters),
  061 (fixer), 063 (default preset).

## Deliverables

- `crates/rglint/src/{main,cli,exit}.rs`.
- `tests/integration/cli.rs` — end-to-end CLI tests using `assert_cmd`:
  - `rglint --format json` on a fixture → exit 1 + JSON on stdout matching
    spec-058 snapshot.
  - `rglint --fix-dry-run` → unified diff, exit 0.
  - `rglint --init` in an empty dir → creates `.rglintrc.toml`.
  - Bad config → exit 2 + error message.

## Interface / API

```rust
#[derive(Parser)]
pub struct Cli {
    pub paths: Vec<PathBuf>,
    #[arg(long)] pub config: Option<PathBuf>,
    #[arg(long, default_value = "pretty")] pub format: Format,
    #[arg(long)] pub fix: bool,
    #[arg(long)] pub fix_dry_run: bool,
    // ...
}
pub fn run(cli: Cli) -> ExitCode;
```

## Behavior

- Positional `PATH`s override `documents` (lint those files directly with the
  discovered config's schema).
- `--config` skips discovery.
- Exit codes per `exit.rs`; `--max-warnings 0` means any warning fails.
- Errors go to stderr; diagnostics go to the chosen reporter's stream
  (pretty/json → stdout; github → stdout; sarif → stdout).

## Testing

- `assert_cmd` integration tests above.
- Exit-code matrix test.

## Risks / Notes

- Keep `main.rs` thin (delegates to `run`) so the integration tests can call
  `run` directly without spawning a process where convenient.

## Implementation notes

Implemented in `crates/rglint` with a thin `main.rs`, public `cli::run`, and
stable `exit::ExitCode` mapping. The CLI resolves `.rglintrc` first and then
the GraphQL-config aliases, preserves the selected config directory for
`ProjectResolver`, and supports direct file/directory positional inputs. Rule
overrides intentionally enable the named built-in rules at warning severity;
external `--rulesdir` remains an accepted no-op stub as specified.

The `--init` template leaves the recommended preset commented out because
spec-063 owns the preset contents. Pretty reporter summary suppression is
implemented as a reporter option so `--quiet` does not affect machine-readable
output. Positional directories are expanded to `.graphql`/`.gql` files before
resolution; unsupported glob expressions are intentionally deferred until a
future CLI path-expansion change.
