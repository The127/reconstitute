# reconstitute

Derive macro that generates a `State` struct and a `reconstitute` constructor for hydrating DDD aggregates from persistent storage.

## Motivation

In Domain-Driven Design, aggregate constructors are domain operations — they enforce invariants and emit domain events. Hydrating an aggregate from a database row is a different concern: no events should fire, no invariants need re-validating, and the aggregate's internal fields must be set directly from the stored values.

The naive solution is to expose a `pub` constructor (or worse, `pub` fields) just to satisfy the persistence layer. That breaks encapsulation: any caller can now bypass domain logic.

`reconstitute` solves this by generating:

1. A companion `{TypeName}State` struct with all the same fields declared `pub` — safe to construct in your repository or read model layer.
2. A `{TypeName}::reconstitute(state: {TypeName}State) -> Self` associated function that moves the state into the aggregate, calling `Default::default()` for any fields explicitly excluded from the state struct.

The aggregate's own fields stay private. The generated `reconstitute` function lives inside `impl {TypeName}`, so it sees the private fields without exposing them to the outside world.

## Features

- Generates a `{TypeName}State` struct with all non-ignored fields exposed as `pub`
- Generates a `{TypeName}::reconstitute(state)` constructor that populates the aggregate from state
- `#[reconstitute_ignore]` attribute excludes fields from `State` and fills them with `Default::default()` at reconstitution time — ideal for in-memory caches, event queues, and other transient fields
- Clear compile errors for unsupported shapes (enums, tuple structs, unit structs, unions)
- Zero runtime cost — pure code generation, no traits to implement, no reflection

## Installation

```toml
[dependencies]
reconstitute = "0.1"
```

## Usage

### Basic example

```rust
use reconstitute::Reconstitute;

#[derive(Reconstitute)]
pub struct Order {
    id: OrderId,
    customer_id: CustomerId,
    status: OrderStatus,
    total_cents: i64,
}
```

The macro generates:

```rust
pub struct OrderState {
    pub id: OrderId,
    pub customer_id: CustomerId,
    pub status: OrderStatus,
    pub total_cents: i64,
}

impl Order {
    pub fn reconstitute(state: OrderState) -> Self {
        Self {
            id: state.id,
            customer_id: state.customer_id,
            status: state.status,
            total_cents: state.total_cents,
        }
    }
}
```

### Ignoring transient fields

Fields annotated with `#[reconstitute_ignore]` are excluded from `State` and filled with `Default::default()` when reconstituting. This is the right tool for pending domain event queues, lazy-computed caches, and anything else that has no persistent representation.

```rust
use reconstitute::Reconstitute;

#[derive(Reconstitute)]
pub struct Order {
    id: OrderId,
    customer_id: CustomerId,
    status: OrderStatus,
    total_cents: i64,

    /// Domain events accumulated during the current unit of work.
    /// Not persisted — reset to an empty Vec on hydration.
    #[reconstitute_ignore]
    pending_events: Vec<DomainEvent>,

    /// Cached line-item count, recomputed on demand.
    #[reconstitute_ignore]
    line_item_cache: Option<Vec<LineItem>>,
}
```

The macro generates:

```rust
pub struct OrderState {
    pub id: OrderId,
    pub customer_id: CustomerId,
    pub status: OrderStatus,
    pub total_cents: i64,
    // pending_events and line_item_cache are absent
}

impl Order {
    pub fn reconstitute(state: OrderState) -> Self {
        Self {
            id: state.id,
            customer_id: state.customer_id,
            status: state.status,
            total_cents: state.total_cents,
            pending_events: Default::default(),   // Vec::new()
            line_item_cache: Default::default(),  // None
        }
    }
}
```

### Using reconstitute in a repository

```rust
// In your infrastructure layer — OrderRow maps directly to a DB query result.
struct OrderRow {
    id: Uuid,
    customer_id: Uuid,
    status: String,
    total_cents: i64,
}

impl OrderRepository {
    pub async fn find_by_id(&self, id: OrderId) -> Result<Option<Order>, RepoError> {
        let row: Option<OrderRow> = sqlx::query_as!(
            OrderRow,
            "SELECT id, customer_id, status, total_cents FROM orders WHERE id = $1",
            id.as_uuid(),
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| {
            Order::reconstitute(OrderState {
                id: OrderId::from(r.id),
                customer_id: CustomerId::from(r.customer_id),
                status: OrderStatus::from_str(&r.status).expect("invalid status in DB"),
                total_cents: r.total_cents,
            })
        }))
    }
}
```

## Design notes

### Why a separate State struct?

The aggregate's fields are private — enforcing that all mutations go through domain methods. A plain `pub` constructor would widen that surface permanently. `State` is the controlled hole: it lives in the infrastructure layer alongside the query that populates it, and it cannot be confused with a domain operation because it has no behaviour.

### How `#[reconstitute_ignore]` works

The macro iterates over the struct's fields and checks each one for a `reconstitute_ignore` attribute. Matching fields are simply omitted from the generated `State` struct definition. In the `reconstitute` body they appear as `field_name: Default::default()`, so the aggregate is always fully initialized without any `Option` wrapping or `unsafe` memory tricks. The ignored field's type must implement `Default`.

### Visibility

The generated `State` struct inherits the visibility of the annotated struct (`pub`, `pub(crate)`, etc.). This ensures it is accessible wherever the aggregate itself is accessible.

## License

MIT
