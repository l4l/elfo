use std::{sync::Arc, time::SystemTime};

use tracing::Level;

use elfo_core::{trace_id::TraceId, ActorMeta};

use crate::formatters::*;

pub(crate) trait Theme {
    type Timestamp: Formatter<SystemTime>;
    type Level: Formatter<Level>;
    type TraceId: Formatter<Option<TraceId>>;
    type ActorMeta: Formatter<Option<Arc<ActorMeta>>>;
    type Payload: Formatter<str>;
    type Location: Formatter<(&'static str, u32)>;
    type Module: Formatter<str>;
}

pub(crate) struct PlainTheme;

impl Theme for PlainTheme {
    type ActorMeta = EmptyIfNone<Arc<ActorMeta>>;
    type Level = Level;
    type Location = Location;
    type Module = Module;
    type Payload = Payload;
    type Timestamp = Rfc3339Weak;
    type TraceId = EmptyIfNone<TraceId>;
}

pub(crate) struct ColoredTheme;

impl Theme for ColoredTheme {
    type ActorMeta = EmptyIfNone<ColoredByHash<Arc<ActorMeta>>>;
    type Level = ColoredLevel;
    type Location = ColoredLocation;
    type Module = ColoredModule;
    type Payload = ColoredPayload;
    type Timestamp = Rfc3339Weak;
    type TraceId = EmptyIfNone<ColoredByHash<TraceId>>;
}
