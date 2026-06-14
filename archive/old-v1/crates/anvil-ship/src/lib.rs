//! Anvil ship + rollback (P9).
//!
//! This crate contains the project-level ship gate, the cascading rollback
//! machinery, and the configurable transport surface. The CLI layer
//! (`anvil ship`, `anvil phase reopen`) is a thin wrapper around these
//! primitives.
//!
//! ## Invariants enforced here
//!
//! - **Audit-store immutability through rollback** — `execute_rollback`
//!   appends new `RollbackEvent` records only; it never modifies, deletes,
//!   or rewrites any existing record. Pinned by
//!   `test_audit_store_immutable_through_rollback`.
//! - **Cascading invalidation** — re-opening a phase invalidates every
//!   transitive dependent computed via `anvil-graph`. Pinned by
//!   `test_rollback_transitive_invalidation`.
//! - **Rotation reset on rollback** — every invalidated dependent's reviewer
//!   rotation resets to position 0 so the full pool's diversity reviews the
//!   fix. Pinned by `test_rollback_resets_rotation_on_dependents`.

pub mod rollback;
pub mod ship;
pub mod transport;

pub use rollback::{
    compute_rollback_plan, execute_rollback, rotation_offset_for_phase, RollbackPlan,
};
pub use ship::{check_all_phases_shipped, check_unresolved_rollbacks, ShipReadiness};
pub use transport::{execute_transport, parse_transport_actions, TransportAction};
