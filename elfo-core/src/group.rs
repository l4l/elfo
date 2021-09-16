use std::{fmt::Debug, future::Future, marker::PhantomData, sync::Arc};

use smallbox::smallbox;

use crate::{
    config::Config,
    context::Context,
    exec::ExecResult,
    object::{Group, Object},
    routers::Router,
    supervisor::Supervisor,
};

#[derive(Debug)]
pub struct ActorGroup<R, C> {
    termination_policy: TerminationPolicy,
    router: R,
    _config: PhantomData<C>,
}

impl ActorGroup<(), ()> {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            termination_policy: TerminationPolicy::default(),
            router: (),
            _config: PhantomData,
        }
    }
}

impl<R, C> ActorGroup<R, C> {
    pub fn config<C1: Config>(self) -> ActorGroup<R, C1> {
        ActorGroup {
            termination_policy: self.termination_policy,
            router: self.router,
            _config: PhantomData,
        }
    }

    /// The behaviour on the `Terminate` message.
    /// `TerminationPolicy::closing` is used by default.
    pub fn termination_policy(mut self, policy: TerminationPolicy) -> Self {
        self.termination_policy = policy;
        self
    }

    pub fn router<R1: Router<C>>(self, router: R1) -> ActorGroup<R1, C> {
        ActorGroup {
            termination_policy: self.termination_policy,
            router,
            _config: self._config,
        }
    }

    pub fn exec<X, O, ER>(self, exec: X) -> Schema
    where
        R: Router<C>,
        X: Fn(Context<C, R::Key>) -> O + Send + Sync + 'static,
        O: Future<Output = ER> + Send + 'static,
        ER: ExecResult,
        C: Config,
    {
        let run = move |ctx: Context, name: String| {
            let addr = ctx.addr();
            let sv = Arc::new(Supervisor::new(
                ctx,
                name,
                exec,
                self.router,
                self.termination_policy,
            ));
            let router = smallbox!(move |envelope| { sv.handle(envelope) });
            Object::new(addr, Group::new(router))
        };

        Schema { run: Box::new(run) }
    }
}

pub struct Schema {
    pub(crate) run: Box<dyn FnOnce(Context, String) -> Object>,
}

/// The behaviour on the `Terminate` message.
#[derive(Debug, Clone)]
pub struct TerminationPolicy {
    pub(crate) stop_spawning: bool,
    pub(crate) close_mailbox: bool,
}

impl Default for TerminationPolicy {
    fn default() -> Self {
        Self::closing()
    }
}

impl TerminationPolicy {
    /// On `Terminate`:
    /// * A supervisor stops spawning new actors.
    /// * New messages are not accepted more.
    /// * Mailboxes are closed.
    ///
    /// This behaviour is used by default.
    pub fn closing() -> Self {
        Self {
            stop_spawning: true,
            close_mailbox: true,
        }
    }

    /// On `Terminate`:
    /// * A supervisor stops spawning new actors.
    /// * The `Terminate` message can be handled by actors manually.
    /// * Mailboxes receive messages (use `Context::close()` to stop it).
    pub fn manually() -> Self {
        Self {
            stop_spawning: true,
            close_mailbox: false,
        }
    }

    // TODO: add `stop_spawning`?
}
