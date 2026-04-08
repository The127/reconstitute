# reconstitute

Derive macro for hydrating DDD aggregates from storage without punching holes in encapsulation.

## The problem

Aggregate constructors are domain operations — they validate invariants and emit events. Loading an aggregate from a database row is not a domain operation: nothing should fire, nothing needs re-validating, and you need to set fields directly from stored values.

The usual workarounds are bad. A `pub` constructor lets any caller bypass domain logic. Public fields are worse. A `From<Row>` impl either lives inside the domain crate (wrong layer) or requires the domain crate to know about your persistence types (worse).

## What this does

Derive `Reconstitute` on a named struct and you get two things:

**A `{TypeName}State` struct** with the same fields, all `pub`. You construct this in your repository from whatever the database gives you.

**A `{TypeName}::reconstitute(state: {TypeName}State) -> Self` associated function** that moves the state into the aggregate. It's generated inside `impl {TypeName}`, so it can see private fields without exposing them.

The aggregate's own fields stay private. The generated function is the only way in.

Fields you don't want persisted — pending event queues, lazy caches — get `#[reconstitute_ignore]`. They're excluded from `State` and filled with `Default::default()` at reconstitution time.

## Installation

```toml
[dependencies]
reconstitute = "0.1"
```

## Usage

```rust
use reconstitute::Reconstitute;

#[derive(Reconstitute)]
pub struct Order {
    id: OrderId,
    customer_id: CustomerId,
    status: OrderStatus,
    total_cents: i64,

    #[reconstitute_ignore]
    pending_events: Vec<DomainEvent>,

    #[reconstitute_ignore]
    line_item_cache: Option<Vec<LineItem>>,
}
```

This generates:

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
            pending_events: Default::default(),
            line_item_cache: Default::default(),
        }
    }
}
```

In a repository:

```rust
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

## Notes

`#[reconstitute_ignore]` fields must implement `Default`. The generated `State` struct inherits the visibility of the annotated struct. Enums, tuple structs, unit structs, and unions are not supported and produce a clear compile error.

Zero runtime cost — it's pure code generation.

## License

MIT
