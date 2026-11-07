//! [`CalendarRepo`]: the store-backed implementation of the calendar.
//!
//! Pattern: Repository + Facade — one small type hides the object store, the envelope and index
//! rules, the formats and the recurrence engine behind the `Calendar` contract. Its writes take turns
//! and tell the feed (Observer). Its collaborators
//! are injected as traits (`ObjectRepository`, `ObjectQuery`, `ActivityLog`, `RecurrenceEngine`,
//! `TaskItems` of fs-tasks),
//! so each can be swapped.

use std::sync::Arc;

use fs_store::Store;
use fs_store::StoreInfo;
use fs_store::traits::{ActivityLog, ObjectQuery, ObjectRepository};
use fs_tasks_core::TaskItems;
use tokio::sync::Mutex;

use super::backend::StoreAdapter;
use crate::feed::Registry;
use crate::recur::{RRuleEngine, RecurrenceEngine};
use crate::time::Zone;

/// Calendar over an object store. Cheap to clone.
#[derive(Clone)]
pub struct CalendarRepo {
    pub(crate) objects: Arc<dyn ObjectRepository>,
    pub(crate) queries: Arc<dyn ObjectQuery>,
    pub(crate) log: Arc<dyn ActivityLog>,
    pub(crate) info: Option<Arc<dyn StoreInfo>>,
    pub(crate) engine: Arc<dyn RecurrenceEngine>,
    pub(crate) tasks: Option<Arc<dyn TaskItems>>,
    pub(crate) actor: String,
    pub(crate) instance: String,
    pub(crate) floating: Zone,
    pub(crate) feed: Arc<Registry>,
    /// Writes take turns, so a read-modify-write is not lost when the backend yields.
    pub(crate) writes: Arc<Mutex<()>>,
}

impl CalendarRepo {
    /// Calendar over a whole [`Store`]. `actor` is the IRI of the acting person / program
    /// (written to the log).
    pub fn new(store: Arc<dyn Store>, actor: impl Into<String>) -> Self {
        let adapter = Arc::new(StoreAdapter(store));
        Self::with_backend(adapter.clone(), adapter.clone(), adapter.clone(), actor)
            .with_info(adapter)
    }

    /// Calendar over the narrow store traits only (a remote or fake backend can supply them).
    pub fn with_backend(
        objects: Arc<dyn ObjectRepository>,
        queries: Arc<dyn ObjectQuery>,
        log: Arc<dyn ActivityLog>,
        actor: impl Into<String>,
    ) -> Self {
        Self {
            objects,
            queries,
            log,
            info: None,
            engine: Arc::new(RRuleEngine),
            tasks: None,
            actor: actor.into(),
            instance: "local.invalid".into(),
            floating: Zone::Utc,
            feed: Arc::default(),
            writes: Arc::default(),
        }
    }

    /// Say whose store this is: [`Calendar::identity`](crate::Calendar::identity) answers from it.
    #[must_use]
    pub fn with_info(mut self, info: Arc<dyn StoreInfo>) -> Self {
        self.info = Some(info);
        self
    }

    /// Instance domain for IRIs `https://<instance>/events/<id>` (ADR 0027).
    #[must_use]
    pub fn with_instance(mut self, instance: impl Into<String>) -> Self {
        self.instance = instance.into();
        self
    }

    /// Zone in which floating events (no `timeZone`) are interpreted for indexing and queries.
    #[must_use]
    pub fn with_floating_zone(mut self, zone: Zone) -> Self {
        self.floating = zone;
        self
    }

    /// Show tasks next to events: the calendar reads them **only** through the `fs-tasks`
    /// interface (ADR 0050); it has no task model or storage of its own.
    #[must_use]
    pub fn with_tasks(mut self, tasks: Arc<dyn TaskItems>) -> Self {
        self.tasks = Some(tasks);
        self
    }

    /// Use another recurrence engine (default: [`RRuleEngine`]).
    #[must_use]
    pub fn with_recurrence_engine(mut self, engine: Arc<dyn RecurrenceEngine>) -> Self {
        self.engine = engine;
        self
    }
}
