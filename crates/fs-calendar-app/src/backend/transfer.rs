//! The other commands: agenda, import, export, invitations, check.
//!
//! Pattern: Command dispatch (continued from `commands.rs`) — each arm calls one method of the `Calendar` trait and
//! shapes its answer into the result fields of the description.

use fs_admin_frontend::{BackendError, Outcome, Values};
use fs_calendar_core::Event;
use fs_calendar_core::{Response, validate};

use super::commands::{done, event_id};
use super::errors::{missing, not_in_list, refusal};
use super::find::range;
use super::{EventBackend, instance_id, iri};

fn text<'a>(input: &'a Values, name: &str) -> Result<&'a str, BackendError> {
    input
        .get(name)
        .and_then(|v| v.first())
        .map(String::as_str)
        .ok_or_else(|| missing(name))
}

fn media_type(format: &str) -> &'static str {
    if format == "jscalendar" {
        "application/jscalendar+json"
    } else {
        "text/calendar"
    }
}

fn answer(pairs: Vec<(&str, Vec<String>)>) -> Outcome {
    Outcome {
        id: None,
        values: pairs.into_iter().map(|(k, v)| (k.to_owned(), v)).collect(),
    }
}

impl EventBackend {
    pub(crate) fn transfer_command(
        &self,
        name: &str,
        input: &Values,
    ) -> Result<Outcome, BackendError> {
        let fail = |e| refusal(&self.description, &e);
        match name {
            "agenda" => {
                let (from, to) = range(input)?.ok_or_else(|| missing("from"))?;
                let agenda = self.block(self.calendar.agenda(from, to)).map_err(fail)?;
                let events = agenda.events.iter().map(instance_id).collect();
                let tasks = agenda.tasks.iter().map(|t| iri(t.id)).collect();
                Ok(answer(vec![("events", events), ("tasks", tasks)]))
            }
            "import" => {
                let report = self
                    .block(
                        self.calendar
                            .import_text(media_type(text(input, "format")?), text(input, "text")?),
                    )
                    .map_err(fail)?;
                Ok(answer(vec![
                    (
                        "created",
                        report.created.iter().map(|id| iri(*id)).collect(),
                    ),
                    ("skipped", report.skipped),
                    ("failed", report.failed),
                    ("notes", report.notes),
                ]))
            }
            "export" => {
                let ids = input
                    .get("ids")
                    .ok_or_else(|| missing("ids"))?
                    .iter()
                    .map(|t| event_id(&Values::from([("id".to_owned(), vec![t.clone()])])))
                    .collect::<Result<Vec<_>, _>>()?;
                let out = self
                    .block(
                        self.calendar
                            .export_text(&ids, media_type(text(input, "format")?)),
                    )
                    .map_err(fail)?;
                Ok(answer(vec![("text", vec![out])]))
            }
            "invite" => {
                let id = event_id(input)?;
                let who = [(text(input, "name")?, text(input, "email")?)];
                self.block(self.calendar.invite(id, &who)).map_err(fail)?;
                Ok(done(id))
            }
            "respond" => self.respond(input),
            "history" => {
                let log = self
                    .block(self.calendar.invitation_log(event_id(input)?))
                    .map_err(fail)?;
                let entries = log
                    .iter()
                    .map(|e| {
                        let who = e.email.as_deref().unwrap_or(&e.participant);
                        let note = e
                            .comment
                            .as_deref()
                            .map(|c| format!(" ({c})"))
                            .unwrap_or_default();
                        format!("{} {} {who}{note}", e.at.to_rfc3339(), e.activity)
                    })
                    .collect();
                Ok(answer(vec![("entries", entries)]))
            }
            "check" => {
                let id = event_id(input)?;
                let stored = self
                    .block(self.calendar.get(id))
                    .map_err(fail)?
                    .ok_or(BackendError::NotFound)?;
                let problems = match Event::from_value(stored.content) {
                    Ok(e) => validate::issues(&e)
                        .iter()
                        .map(ToString::to_string)
                        .collect(),
                    Err(e) => vec![e.to_string()],
                };
                Ok(answer(vec![("problems", problems)]))
            }
            _ => Err(BackendError::NotFound),
        }
    }
}

impl EventBackend {
    /// `respond`: an unknown answer is refused, never read as a rejection.
    fn respond(&self, input: &Values) -> Result<Outcome, BackendError> {
        let id = event_id(input)?;
        let response = match text(input, "response")? {
            "accept" => Response::Accept,
            "tentative-accept" => Response::TentativeAccept,
            "reject" => Response::Reject,
            _ => return Err(not_in_list("response")),
        };
        let comment = input
            .get("comment")
            .and_then(|v| v.first())
            .map(String::as_str);
        self.block(
            self.calendar
                .respond(id, text(input, "who")?, response, comment),
        )
        .map_err(|e| refusal(&self.description, &e))?;
        Ok(done(id))
    }
}
