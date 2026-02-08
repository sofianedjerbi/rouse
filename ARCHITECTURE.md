# Rouse — Architecture Document

> Open-source on-call management platform.
> "Rouse wakes up the right person."

## 1. What Rouse Does

Rouse receives alerts from monitoring tools, determines who is on-call,
and pages them through multiple channels (Slack, Discord, Telegram,
WhatsApp, SMS, phone, email). If nobody responds, it escalates.
It tracks on-call health and helps teams reduce alert noise.

## 2. Design Principles

1. **Single binary** — `./rouse` starts everything. No Redis, no RabbitMQ, no Celery.
2. **SQLite by default** — zero-config. `./rouse` creates `rouse.db` and runs.
3. **PostgreSQL optional** — flip a flag for HA/multi-instance.
4. **Hexagonal architecture** — domain core has zero external dependencies. All I/O goes through ports (traits).
5. **Pluggable everything** — alert sources (inbound adapters) and notification channels (outbound adapters) are traits. Adding a new integration = one file.
6. **The database is the queue** — no external message broker. Pending notifications and escalation deadlines are rows in the DB, processed by background tokio tasks.

## 3. High-Level Architecture

```
                    ┌─────────────────────────────────────┐
                    │          Inbound Adapters            │
                    │                                     │
                    │  Alertmanager  Grafana  Datadog     │
                    │  CloudWatch    Generic Webhook      │
                    └────────────────┬────────────────────┘
                                     │ HTTP POST
                                     ▼
┌────────────────────────────────────────────────────────────────┐
│                        ROUSE BINARY                            │
│                                                                │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │                    HTTP Server (axum)                     │  │
│  │                                                          │  │
│  │  /healthz                  → liveness probe               │  │
│  │  /readyz                   → readiness probe              │  │
│  │  /metrics                  → Prometheus metrics            │  │
│  │  /api/webhooks/{source}    → alert ingestion (idempotent) │  │
│  │  /api/alerts               → CRUD + ack/resolve           │  │
│  │  /api/schedules            → schedule management          │  │
│  │  /api/escalations          → policy management            │  │
│  │  /api/health               → on-call health metrics       │  │
│  │  /api/integrations         → channel config               │  │
│  │  /*                        → SvelteKit static UI          │  │
│  └──────────────────────┬───────────────────────────────────┘  │
│                         │                                      │
│  ┌──────────────────────▼───────────────────────────────────┐  │
│  │                Application Services                       │  │
│  │                                                          │  │
│  │  AlertService          → receive, dedup, route           │  │
│  │  ScheduleService       → who is on-call                  │  │
│  │  EscalationService     → step progression                │  │
│  │  HealthService         → burnout, noise, fairness        │  │
│  └──────────────────────┬───────────────────────────────────┘  │
│                         │                                      │
│  ┌──────────────────────▼───────────────────────────────────┐  │
│  │                   Domain Core                             │  │
│  │                   (pure Rust, no async, no I/O)           │  │
│  │                                                          │  │
│  │  Alert         → aggregate root + state machine          │  │
│  │  Schedule      → rotation, overrides, WhoIsOnCall()      │  │
│  │  Escalation    → policy, steps, timing                   │  │
│  │  User / Team   → entities                                │  │
│  │  Events        → AlertReceived, Escalated, Resolved...   │  │
│  └──────────────────────────────────────────────────────────┘  │
│                         │                                      │
│  ┌──────────────────────▼───────────────────────────────────┐  │
│  │                   Ports (traits)                           │  │
│  │                                                          │  │
│  │  Inbound:   AlertReceiver, AlertManager, ScheduleManager │  │
│  │  Outbound:  Notifier, Repository, EventPublisher         │  │
│  └──────────────────────┬───────────────────────────────────┘  │
│                         │                                      │
│  ┌──────────────────────▼───────────────────────────────────┐  │
│  │                Outbound Adapters                          │  │
│  │                                                          │  │
│  │  Notifiers:    Slack · Discord · Telegram · WhatsApp     │  │
│  │                Twilio SMS · Twilio Voice · Email          │  │
│  │                Generic Webhook                           │  │
│  │                                                          │  │
│  │  Persistence:  SQLite (default) · PostgreSQL (optional)  │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │                Background Workers (tokio)                 │  │
│  │                                                          │  │
│  │  NotificationWorker   → polls pending notifications      │  │
│  │  EscalationWorker     → polls pending escalation steps   │  │
│  │  HealthCollector      → aggregates on-call metrics       │  │
│  └──────────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────────┘
         │
         ▼
   ┌───────────┐
   │  SQLite   │  (default — single file, zero config)
   │    or     │
   │ PostgreSQL│  (optional — for HA / multi-instance)
   └───────────┘
```

## 4. Directory Structure

```
rouse/
├── Cargo.toml
├── Cargo.lock
│
├── crates/
│   ├── rouse-core/              # Domain core — pure Rust, zero dependencies
│   │   ├── Cargo.toml           # no external deps except uuid, chrono
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── alert/
│   │       │   ├── mod.rs       # Alert aggregate root
│   │       │   ├── severity.rs  # value object
│   │       │   ├── status.rs    # value object + state machine
│   │       │   └── fingerprint.rs
│   │       ├── schedule/
│   │       │   ├── mod.rs            # Schedule aggregate root + WhoIsOnCall
│   │       │   ├── rotation.rs      # daily / weekly / custom
│   │       │   └── shift_override.rs  # temporary override (override is a Rust keyword)
│   │       ├── escalation/
│   │       │   ├── mod.rs       # EscalationPolicy aggregate root
│   │       │   ├── step.rs      # EscalationStep value object
│   │       │   └── target.rs    # who to notify
│   │       ├── user/
│   │       │   ├── mod.rs       # User + Team entities
│   │       │   └── phone.rs     # E.164 validation
│   │       └── event.rs         # domain events
│   │
│   ├── rouse-ports/             # Trait definitions (hexagonal boundary)
│   │   ├── Cargo.toml           # depends only on rouse-core
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── inbound.rs       # AlertReceiver, AlertManager, ScheduleManager
│   │       └── outbound.rs      # Notifier, Repository, EventPublisher
│   │
│   ├── rouse-app/               # Application services (use cases)
│   │   ├── Cargo.toml           # depends on rouse-core + rouse-ports
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── alert_service.rs
│   │       ├── schedule_service.rs
│   │       ├── escalation_service.rs
│   │       ├── health_service.rs
│   │       └── router.rs        # label matching → escalation policy
│   │
│   ├── rouse-adapters/          # All adapter implementations
│   │   ├── Cargo.toml           # depends on rouse-ports, external crates
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── inbound/
│   │       │   ├── mod.rs
│   │       │   ├── alertmanager.rs   # Alertmanager webhook parser
│   │       │   ├── grafana.rs        # Grafana webhook parser
│   │       │   ├── datadog.rs        # Datadog webhook parser
│   │       │   └── generic.rs        # Generic JSON webhook
│   │       ├── outbound/
│   │       │   ├── mod.rs
│   │       │   ├── slack.rs          # Slack (Bot API + interactive)
│   │       │   ├── discord.rs        # Discord (Bot API)
│   │       │   ├── telegram.rs       # Telegram (Bot API)
│   │       │   ├── whatsapp.rs       # WhatsApp (Twilio/Meta API)
│   │       │   ├── twilio_sms.rs     # Twilio SMS
│   │       │   ├── twilio_voice.rs   # Twilio Voice (TwiML)
│   │       │   ├── email.rs          # SMTP
│   │       │   └── webhook.rs        # Generic outbound webhook
│   │       └── persistence/
│   │           ├── mod.rs
│   │           ├── sqlite.rs         # SQLite repositories
│   │           └── postgres.rs       # PostgreSQL repositories
│   │
│   └── rouse-server/            # HTTP server + background workers + main
│       ├── Cargo.toml           # depends on everything, axum, tokio
│       └── src/
│           ├── main.rs          # CLI args, config, wiring, startup
│           ├── api/
│           │   ├── mod.rs
│           │   ├── webhooks.rs      # POST /api/webhooks/{source}
│           │   ├── alerts.rs        # alerts CRUD + ack/resolve
│           │   ├── schedules.rs     # schedules CRUD
│           │   ├── escalations.rs   # policies CRUD
│           │   ├── health.rs        # health metrics endpoint
│           │   └── integrations.rs  # channel configuration
│           ├── workers/
│           │   ├── mod.rs
│           │   ├── notification.rs  # polls + sends pending notifications
│           │   ├── escalation.rs    # polls + fires pending escalation steps
│           │   └── health.rs        # aggregates on-call metrics
│           └── config.rs            # YAML config + CLI args + env vars
│
├── ui/                          # SvelteKit frontend
│   ├── package.json
│   ├── svelte.config.js
│   └── src/
│       ├── routes/
│       │   ├── +page.svelte         # Dashboard
│       │   ├── alerts/
│       │   ├── schedules/
│       │   ├── escalations/
│       │   ├── integrations/
│       │   └── health/
│       └── lib/
│           └── api.ts               # API client
│
├── deploy/
│   ├── Dockerfile               # multi-stage: build Rust + SvelteKit
│   ├── docker-compose.yaml      # one-command deploy
│   ├── k8s/
│   │   ├── statefulset.yaml     # SQLite mode
│   │   └── deployment.yaml      # PostgreSQL mode
│   └── helm/
│       └── rouse/
│
├── config/
│   └── rouse.example.yaml       # example GitOps config
│
└── tests/
    ├── integration/             # full stack tests with test DB
    └── e2e/                     # Playwright tests for UI
```

## 5. Dependency Direction (Hexagonal Rules)

```
rouse-server ──→ rouse-app ──→ rouse-ports ──→ rouse-core
     │                              ▲
     │                              │
     └──→ rouse-adapters ───────────┘

RULE: rouse-core imports NOTHING from the workspace.
RULE: rouse-ports imports ONLY rouse-core.
RULE: rouse-app imports rouse-core + rouse-ports (traits only).
RULE: rouse-adapters imports rouse-ports (implements traits) + external crates.
RULE: rouse-server wires everything together (dependency injection).
```

This is enforced by Cargo workspace — crates can only import what's in
their `[dependencies]`. The domain core literally cannot depend on SQLite,
axum, or Twilio. If someone tries, `cargo build` fails.

## 6. The Database-as-Queue Pattern

No external message broker. Two tables drive all async work:

### notifications table
```sql
CREATE TABLE notifications (
    id          TEXT PRIMARY KEY,
    alert_id    TEXT NOT NULL REFERENCES alerts(id),
    channel     TEXT NOT NULL,          -- "slack", "sms", "phone", etc.
    target      TEXT NOT NULL,          -- user ID or channel ID
    payload     TEXT NOT NULL,          -- JSON notification content
    status      TEXT NOT NULL DEFAULT 'pending',  -- pending/sent/failed/dead
    next_attempt_at DATETIME NOT NULL,
    retry_count INTEGER NOT NULL DEFAULT 0,
    created_at  DATETIME NOT NULL,
    sent_at     DATETIME,
    error       TEXT                    -- last error message if failed
);

CREATE INDEX idx_notifications_pending
    ON notifications(status, next_attempt_at)
    WHERE status = 'pending';
```

### escalation_steps table
```sql
CREATE TABLE escalation_steps (
    id          TEXT PRIMARY KEY,
    alert_id    TEXT NOT NULL REFERENCES alerts(id),
    policy_id   TEXT NOT NULL REFERENCES escalation_policies(id),
    step_order  INTEGER NOT NULL,
    fires_at    DATETIME NOT NULL,
    status      TEXT NOT NULL DEFAULT 'pending',  -- pending/fired/cancelled
    created_at  DATETIME NOT NULL
);

CREATE INDEX idx_escalations_pending
    ON escalation_steps(status, fires_at)
    WHERE status = 'pending';
```

### Worker loop (simplified)
```rust
// Runs every 2 seconds
async fn notification_worker(db: &Pool, notifiers: &NotifierRegistry) {
    loop {
        let pending = db.query(
            "SELECT * FROM notifications
             WHERE status = 'pending' AND next_attempt_at <= ?",
            now()
        ).await;

        for n in pending {
            match notifiers.get(&n.channel).send(&n).await {
                Ok(_) => db.update_status(n.id, "sent").await,
                Err(e) => {
                    if n.retry_count >= MAX_RETRIES {
                        db.update_status(n.id, "dead").await;
                    } else {
                        db.update_retry(n.id, n.retry_count + 1, backoff(n.retry_count)).await;
                    }
                }
            }
        }

        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}
```

## 7. Configuration

Three layers, merged in order (last wins):

1. **YAML file** (`rouse.yaml`) — GitOps-friendly, lives in your repo
2. **Environment variables** (`ROUSE_DATABASE_URL`, `ROUSE_SLACK_TOKEN`, etc.)
3. **CLI flags** (`--database-url`, `--port`, etc.)

```yaml
# rouse.yaml
server:
  port: 8080
  host: 0.0.0.0

database:
  url: sqlite:///data/rouse.db    # or postgres://...

schedules:
  platform-team:
    rotation: weekly
    timezone: Europe/Zurich
    participants: [alice, bob, charlie]
    handoff: monday 09:00

escalation_policies:
  platform-critical:
    steps:
      - wait: 0m
        notify: on-call(platform-team)
        channels: [slack, sms]
      - wait: 10m
        notify: on-call(platform-team, next)
        channels: [slack, sms, phone]
      - wait: 20m
        notify: engineering-manager
        channels: [phone]
    repeat: 1

routes:
  - match: { severity: critical, service: payments }
    policy: platform-critical
  - match: { severity: warning }
    policy: platform-low
    suppress_hours: ["22:00-08:00"]

integrations:
  slack:
    bot_token: ${ROUSE_SLACK_BOT_TOKEN}
    app_token: ${ROUSE_SLACK_APP_TOKEN}
  twilio:
    account_sid: ${ROUSE_TWILIO_SID}
    auth_token: ${ROUSE_TWILIO_TOKEN}
    from_number: "+1234567890"
```

## 8. Deployment Models

### Minimal (laptop / small team)
```bash
curl -L https://getrouse.io/install | sh
./rouse
# → listening on :8080, SQLite at ./rouse.db
```

### Docker Compose
```yaml
services:
  rouse:
    image: ghcr.io/rousedev/rouse:latest
    ports: ["8080:8080"]
    volumes: ["./data:/data"]
    environment:
      ROUSE_SLACK_BOT_TOKEN: xoxb-...
```

### Kubernetes — SQLite (single instance)
```yaml
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: rouse
spec:
  replicas: 1
  template:
    spec:
      containers:
        - name: rouse
          image: ghcr.io/rousedev/rouse:latest
          volumeMounts:
            - name: data
              mountPath: /data
  volumeClaimTemplates:
    - metadata:
        name: data
      spec:
        accessModes: ["ReadWriteOnce"]
        resources:
          requests:
            storage: 1Gi
```

### Kubernetes — PostgreSQL (HA)
```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: rouse
spec:
  replicas: 3
  template:
    spec:
      containers:
        - name: rouse
          image: ghcr.io/rousedev/rouse:latest
          env:
            - name: ROUSE_DATABASE_URL
              value: postgres://rouse:pass@postgres:5432/rouse
```

## 9. Rust Crate Dependencies

### rouse-core (minimal)
```toml
[dependencies]
uuid = { version = "1", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }
serde = { version = "1", features = ["derive"] }
thiserror = "2"
```

### rouse-ports
```toml
[dependencies]
rouse-core = { path = "../rouse-core" }
async-trait = "0.1"   # needed for dyn-safe async traits
```

### rouse-adapters
```toml
[dependencies]
rouse-ports = { path = "../rouse-ports" }
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite", "postgres"] }
reqwest = { version = "0.12", features = ["json"] }
serde_json = "1"
tracing = "0.1"
```

### rouse-server
```toml
[dependencies]
rouse-core = { path = "../rouse-core" }
rouse-ports = { path = "../rouse-ports" }
rouse-app = { path = "../rouse-app" }
rouse-adapters = { path = "../rouse-adapters" }
axum = "0.8"
tokio = { version = "1", features = ["full"] }
tokio-util = "0.7"                                     # CancellationToken
tower = "0.5"
tower-http = { version = "0.6", features = ["fs", "cors", "trace"] }
clap = { version = "4", features = ["derive"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["json"] }
metrics = "0.24"                                        # Prometheus metrics
metrics-exporter-prometheus = "0.16"
```

## 10. SaaS vs Self-Hosted

The exact same binary. The difference is configuration:

| | Self-Hosted | SaaS (getrouse.io) |
|---|---|---|
| Database | Your SQLite/Postgres | Managed PostgreSQL |
| Slack | Your bot token | Pre-configured OAuth |
| SMS/Phone | Your Twilio keys | Built-in, metered |
| Discord | Your bot token | Pre-configured |
| WhatsApp | Your Meta/Twilio setup | Built-in |
| Billing | Free | Per-seat subscription |
| Multi-tenancy | Single tenant | Multi-tenant (tenant_id on all tables) |

The SaaS adds one layer: a `tenant_id` column on every table + auth middleware
that scopes all queries to the authenticated tenant. Same core, same adapters.
