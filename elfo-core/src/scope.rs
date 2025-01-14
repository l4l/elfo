use std::{cell::Cell, future::Future, sync::Arc};

use crate::{
    actor::ActorMeta,
    addr::Addr,
    config::SystemConfig,
    dumping::DumpingControl,
    logging::_priv::LoggingControl,
    permissions::{AtomicPermissions, Permissions},
    tracing::TraceId,
};

tokio::task_local! {
    static SCOPE: Scope;
}

#[derive(Clone)]
pub struct Scope {
    actor: Addr,
    meta: Arc<ActorMeta>,
    trace_id: Cell<TraceId>,
    shared: Arc<ScopeShared>,
    allocated_bytes: Cell<usize>,
    deallocated_bytes: Cell<usize>,
}

assert_impl_all!(Scope: Send);
assert_not_impl_all!(Scope: Sync);

impl Scope {
    /// Private API for now.
    #[doc(hidden)]
    pub fn test(actor: Addr, meta: Arc<ActorMeta>) -> Self {
        Self::new(
            TraceId::generate(),
            actor,
            meta,
            Arc::new(ScopeShared::new(Addr::NULL)),
        )
    }

    pub(crate) fn new(
        trace_id: TraceId,
        actor: Addr,
        meta: Arc<ActorMeta>,
        shared: Arc<ScopeShared>,
    ) -> Self {
        Self {
            actor,
            meta,
            trace_id: Cell::new(trace_id),
            shared,
            allocated_bytes: Cell::new(0),
            deallocated_bytes: Cell::new(0),
        }
    }

    #[inline]
    #[deprecated(note = "use `actor()` instead")]
    pub fn addr(&self) -> Addr {
        self.actor
    }

    #[inline]
    pub fn actor(&self) -> Addr {
        self.actor
    }

    #[inline]
    pub fn group(&self) -> Addr {
        self.shared.group
    }

    /// Returns the current object's meta.
    #[inline]
    pub fn meta(&self) -> &Arc<ActorMeta> {
        &self.meta
    }

    /// Returns the current trace id.
    #[inline]
    pub fn trace_id(&self) -> TraceId {
        self.trace_id.get()
    }

    /// Replaces the current trace id with the provided one.
    #[inline]
    pub fn set_trace_id(&self, trace_id: TraceId) {
        self.trace_id.set(trace_id);
    }

    /// Returns the current permissions (for logging, telemetry and so on).
    #[inline]
    pub fn permissions(&self) -> Permissions {
        self.shared.permissions.load()
    }

    /// Private API for now.
    #[inline]
    #[stability::unstable]
    #[doc(hidden)]
    pub fn logging(&self) -> &LoggingControl {
        &self.shared.logging
    }

    /// Private API for now.
    #[inline]
    #[stability::unstable]
    #[doc(hidden)]
    pub fn dumping(&self) -> &DumpingControl {
        &self.shared.dumping
    }

    #[doc(hidden)]
    #[stability::unstable]
    pub fn increment_allocated_bytes(&self, by: usize) {
        self.allocated_bytes.set(self.allocated_bytes.get() + by);
    }

    #[doc(hidden)]
    #[stability::unstable]
    pub fn increment_deallocated_bytes(&self, by: usize) {
        self.deallocated_bytes
            .set(self.deallocated_bytes.get() + by);
    }

    pub(crate) fn take_allocated_bytes(&self) -> usize {
        self.allocated_bytes.take()
    }

    pub(crate) fn take_deallocated_bytes(&self) -> usize {
        self.deallocated_bytes.take()
    }

    /// Wraps the provided future with the current scope.
    pub async fn within<F: Future>(self, f: F) -> F::Output {
        SCOPE.scope(self, f).await
    }

    /// Runs the provided function with the current scope.
    pub fn sync_within<R>(self, f: impl FnOnce() -> R) -> R {
        SCOPE.sync_scope(self, f)
    }
}

pub(crate) struct ScopeShared {
    group: Addr,
    permissions: AtomicPermissions,
    logging: LoggingControl,
    dumping: DumpingControl,
}

impl ScopeShared {
    pub(crate) fn new(group: Addr) -> Self {
        Self {
            group,
            permissions: Default::default(), // everything is disabled
            logging: Default::default(),
            dumping: Default::default(),
        }
    }

    pub(crate) fn configure(&self, config: &SystemConfig) {
        // Update the logging subsystem.
        self.logging.configure(&config.logging);
        let max_level = self.logging.max_level_hint().into_level();

        // Update the dumping subsystem.
        self.dumping.configure(&config.dumping);

        // Update permissions.
        let mut perm = self.permissions.load();
        perm.set_logging_enabled(max_level);
        perm.set_dumping_enabled(!config.dumping.disabled);
        perm.set_telemetry_per_actor_group_enabled(config.telemetry.per_actor_group);
        perm.set_telemetry_per_actor_key_enabled(config.telemetry.per_actor_key);
        self.permissions.store(perm);
    }
}

/// Exposes the current scope in order to send to other tasks.
///
/// # Panics
/// This function will panic if called outside actors.
pub fn expose() -> Scope {
    SCOPE.with(Clone::clone)
}

/// Exposes the current scope if inside the actor system.
pub fn try_expose() -> Option<Scope> {
    SCOPE.try_with(Clone::clone).ok()
}

/// Accesses the current scope and runs the provided closure.
///
/// # Panics
/// This function will panic if called ouside the actor system.
#[inline]
pub fn with<R>(f: impl FnOnce(&Scope) -> R) -> R {
    try_with(f).expect("cannot access a scope outside the actor system")
}

/// Accesses the current scope and runs the provided closure.
///
/// Returns `None` if called outside the actor system.
/// For a panicking variant, see `with`.
#[inline]
pub fn try_with<R>(f: impl FnOnce(&Scope) -> R) -> Option<R> {
    SCOPE.try_with(|scope| f(scope)).ok()
}

/// Returns the current trace id.
///
/// # Panics
/// This function will panic if called ouside the actor system.
#[inline]
pub fn trace_id() -> TraceId {
    with(Scope::trace_id)
}

/// Returns the current trace id if inside the actor system.
#[inline]
pub fn try_trace_id() -> Option<TraceId> {
    try_with(Scope::trace_id)
}

/// Replaces the current trace id with the provided one.
///
/// # Panics
/// This function will panic if called ouside the actor system.
#[inline]
pub fn set_trace_id(trace_id: TraceId) {
    with(|scope| scope.set_trace_id(trace_id));
}

/// Replaces the current trace id with the provided one
/// if inside the actor system.
///
/// Returns `true` if the trace id has been replaced.
#[inline]
pub fn try_set_trace_id(trace_id: TraceId) -> bool {
    try_with(|scope| scope.set_trace_id(trace_id)).is_some()
}

/// Returns the current object's meta.
///
/// # Panics
/// This function will panic if called ouside the actor system.
#[inline]
pub fn meta() -> Arc<ActorMeta> {
    with(|scope| scope.meta().clone())
}

/// Returns the current object's meta if inside the actor system.
#[inline]
pub fn try_meta() -> Option<Arc<ActorMeta>> {
    try_with(|scope| scope.meta().clone())
}
