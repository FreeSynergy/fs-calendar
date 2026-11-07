//! `Items` interface: every method of the contract, each one call of the provider.
//!
//! Pattern: Adapter — wire request in, `Calendar` call, wire response out. What a provider cannot
//! do arrives as its explicit `Unsupported` and leaves as a typed problem.

use std::rc::Rc;
use std::sync::Arc;

use fs_calendar_core::Calendar;
use fs_calendar_core::Revision;
use fs_interfaces::events_capnp::items;
use fs_interfaces::rpc::{check_answer, invalid_request, read_object_id, run_to_end, too_large};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::errors::rpc_error;
use crate::report::write_report;
use crate::wire::{
    read_event, read_ids, read_time, required, write_event, write_instances, write_page,
};

pub(crate) struct ItemsImpl {
    pub(crate) calendar: Arc<dyn Calendar>,
    /// Turns for the requests that write events (see `MAX_CONCURRENT_WRITES`).
    pub(crate) writers: Arc<Semaphore>,
}

impl ItemsImpl {
    /// A turn to read and write an event; held until the request is done.
    async fn turn(&self) -> capnp::Result<OwnedSemaphorePermit> {
        self.writers
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| capnp::Error::failed("the server is stopping".into()))
    }
}

type Done = Result<(), capnp::Error>;

impl items::Server for ItemsImpl {
    async fn create(
        self: Rc<Self>,
        params: items::CreateParams,
        mut results: items::CreateResults,
    ) -> Done {
        let _turn = self.turn().await?;
        let event = read_event(params.get()?.get_jscalendar()?)?;
        let calendar = self.calendar.clone();
        let object = run_to_end(async move { calendar.create(event).await })
            .await?
            .map_err(rpc_error)?;
        write_event(results.get().init_event(), &object);
        Ok(())
    }

    async fn update(
        self: Rc<Self>,
        params: items::UpdateParams,
        mut results: items::UpdateResults,
    ) -> Done {
        let _turn = self.turn().await?;
        let p = params.get()?;
        let id = read_object_id(p.get_id()?)?;
        let event = read_event(p.get_jscalendar()?)?;
        let calendar = self.calendar.clone();
        let object = run_to_end(async move { calendar.update(id, event).await })
            .await?
            .map_err(rpc_error)?;
        write_event(results.get().init_event(), &object);
        Ok(())
    }

    async fn update_if(
        self: Rc<Self>,
        params: items::UpdateIfParams,
        mut results: items::UpdateIfResults,
    ) -> Done {
        let _turn = self.turn().await?;
        let p = params.get()?;
        let id = read_object_id(p.get_id()?)?;
        let revision = Revision::from(p.get_revision());
        let event = read_event(p.get_jscalendar()?)?;
        let calendar = self.calendar.clone();
        let object = run_to_end(async move { calendar.update_checked(id, &revision, event).await })
            .await?
            .map_err(rpc_error)?;
        write_event(results.get().init_event(), &object);
        Ok(())
    }

    async fn delete(self: Rc<Self>, params: items::DeleteParams, _: items::DeleteResults) -> Done {
        let id = read_object_id(params.get()?.get_id()?)?;
        let calendar = self.calendar.clone();
        run_to_end(async move { calendar.delete(id).await })
            .await?
            .map_err(rpc_error)
    }

    async fn get(self: Rc<Self>, params: items::GetParams, mut results: items::GetResults) -> Done {
        let id = read_object_id(params.get()?.get_id()?)?;
        let found = self.calendar.get(id).await.map_err(rpc_error)?;
        let mut r = results.get();
        r.set_found(found.is_some());
        if let Some(object) = found {
            write_event(r.init_event(), &object);
        }
        Ok(())
    }

    async fn get_by_uid(
        self: Rc<Self>,
        params: items::GetByUidParams,
        mut results: items::GetByUidResults,
    ) -> Done {
        let uid = required(params.get()?.get_uid()?, "uid")?;
        let found = self.calendar.get_by_uid(uid).await.map_err(rpc_error)?;
        let mut r = results.get();
        r.set_found(found.is_some());
        if let Some(object) = found {
            write_event(r.init_event(), &object);
        }
        Ok(())
    }

    async fn list(
        self: Rc<Self>,
        params: items::ListParams,
        mut results: items::ListResults,
    ) -> Done {
        let all = self.calendar.list_events().await.map_err(rpc_error)?;
        write_page(results.get().init_page(), all, params.get()?.get_cursor())
    }

    async fn occurrences(
        self: Rc<Self>,
        params: items::OccurrencesParams,
        results: items::OccurrencesResults,
    ) -> Done {
        let p = params.get()?;
        let (from, to) = (read_time(p.get_from())?, read_time(p.get_to())?);
        if to < from {
            return Err(invalid_request("the window ends before it starts"));
        }
        let window = self.calendar.window(from, to).await.map_err(rpc_error)?;
        // An answer the core had to cut is not sent shorter without a word: the contract has no place
        // to say it, so the consumer is told to ask for less.
        if let Some(cut) = window.damaged.iter().find(|d| d.cut) {
            return Err(too_large(format!(
                "{}; ask for a shorter window",
                cut.reason
            )));
        }
        // The contract has no place for what is damaged either: it is said here, not lost.
        for d in &window.damaged {
            eprintln!(
                "fs-calendar-server: occurrences: event {}{}: {}",
                d.object,
                d.key.as_ref().map_or(String::new(), |k| format!(" at {k}")),
                d.reason
            );
        }
        write_instances(results, &window.instances)
    }

    async fn import_text(
        self: Rc<Self>,
        params: items::ImportTextParams,
        mut results: items::ImportTextResults,
    ) -> Done {
        let _turn = self.turn().await?;
        let p = params.get()?;
        let media = required(p.get_media_type()?, "mediaType")?.to_string();
        let text = crate::wire::optional(p.get_text()?)?.to_string();
        let calendar = self.calendar.clone();
        let report = run_to_end(async move { calendar.import_text(&media, &text).await })
            .await?
            .map_err(rpc_error)?;
        write_report(results.get().init_report(), &report)
    }

    async fn export_text(
        self: Rc<Self>,
        params: items::ExportTextParams,
        mut results: items::ExportTextResults,
    ) -> Done {
        let p = params.get()?;
        let ids = read_ids(p.get_ids()?)?;
        let media = required(p.get_media_type()?, "mediaType")?;
        let text = self
            .calendar
            .export_text(&ids, media)
            .await
            .map_err(rpc_error)?;
        check_answer(text.len())?;
        results.get().set_text(text.as_str());
        Ok(())
    }
}
