# Rouse

Open-source on-call management. Rust backend, SvelteKit frontend, hexagonal architecture.

## Workflow

- **Every feature = GitHub issue first.** `gh issue create` before writing code.
- **Conventional commits.** `feat:`, `fix:`, `refactor:`, `test:`, `docs:`, `chore:`.
- **Small commits.** One concern per commit. Short description, no body needed.
- **TDD.** Write failing test → make it pass → refactor. No production code without a test.
- **Run checks before committing.** `cargo test && cargo clippy -- -D warnings && cargo fmt --check`

## Architecture

```
rouse-core     → pure Rust, no async, no I/O. Only: uuid, chrono, chrono-tz, serde, thiserror.
rouse-ports    → traits only. Depends on rouse-core.
rouse-app      → application services with generics. Depends on core + ports.
rouse-adapters → implements port traits. Depends on rouse-ports ONLY. Never import rouse-app.
rouse-server   → wires everything. Depends on all crates.
```

## Domain Rules

- Aggregates return `Result<Vec<DomainEvent>, DomainError>`.
- `DomainEvent` is an enum, not trait objects.
- App services use generics (`AlertService<R: AlertRepository>`), not `Arc<dyn>`.
- `dyn` only for heterogeneous collections (NotifierRegistry).
- Pass `now: DateTime<Utc>` explicitly. Never call `Utc::now()` in domain.
- `override` is a Rust keyword — use `shift_override.rs`.

## Error Hierarchy

```
DomainError: AlertAlreadyResolved, ScheduleRequiresParticipant, InvalidPhoneFormat,
             InvalidOverridePeriod, InvalidId(String), PolicyRequiresStep,
             StepRequiresTarget, StepRequiresChannel
PortError:   NotFound, Persistence(String), Connection(String)
NotifyError: ChannelUnavailable, RateLimited, InvalidTarget, DeliveryFailed(String)
ParseError:  InvalidJson, MissingField(String), InvalidPayload(String)
AppError:    Domain(DomainError), Port(PortError), Parse(ParseError), Routing(String)
```

## Conventions

- `cargo clippy -- -D warnings` and `cargo fmt` must pass.
- Tests: AAA pattern, behavior-named (`acknowledge_firing_alert_transitions_to_acknowledged`).
- REST: plural nouns, `?page=1&per_page=50`, JSON envelope `{ "data": [], "total": N }`.
- Notifier::notify() returns `NotifyResult` (not `()`). Contains external_id + metadata.
- AlertSourceParser returns `Vec<RawAlert>` (batched webhooks).
