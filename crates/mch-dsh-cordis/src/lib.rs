//! mch-dsh-cordis — the Cordis plugin kernel, forged from DeepSeek Harness.
//!
//! Implements the core plugin framework: the fiber lifecycle
//! (PENDING -> LOADING -> ACTIVE -> UNLOADING -> DISPOSED), dependency
//! injection, a cleanup-aware effect system, the plugin registry, and scoped
//! contexts. Forged from DeepSeek Harness `vendor/cordis/`.

pub mod config;
pub mod context;
pub mod events;
pub mod fiber;
pub mod logger;
pub mod plugin;
pub mod reflect;
pub mod registry;
pub mod service;
pub mod utils;

pub use config::{ConfigSchema, ConfigValidator, ConfigValidatorResult};
pub use context::{Context, InterceptConfig};
pub use events::{EventBus, Waterfall, WaterfallResult};
pub use fiber::{Fiber, FiberError, FiberState};
pub use logger::{LogLevel, Logger};
pub use plugin::{resolve_inject, Inject, Plugin, PluginConfig, PluginRuntime, PluginShape};
pub use reflect::{DynService, Impl, InjectKey, Provide};
pub use registry::Registry;
pub use service::{BaseService, Service};
pub use utils::{build_outer_stack, Disposable, DisposableList, EffectMeta};

/// Re-export commonly used types.
pub mod prelude {
    pub use crate::{
        Context, Fiber, FiberError, FiberState, Inject, Plugin, PluginRuntime, Registry,
    };
}
