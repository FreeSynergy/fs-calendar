//! Splitting an `.ics` stream into per-UID parts of `VEVENT` components.
//!
//! Pattern: Iterator-style scan — one pass over the lines; every part keeps the exact source
//! text (header, `VTIMEZONE`s, the UID's components) so the original can be stored byte for byte (ADR 0030).

use std::collections::BTreeMap;

/// The part of an `.ics` stream that belongs to one `UID` (master + its overrides, with the
/// calendar header and all `VTIMEZONE`s).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IcsPart {
    /// The `UID` shared by the part's components.
    pub uid: String,
    /// Exact source text: the whole input if it holds one UID, otherwise header + `VTIMEZONE`s +
    /// the UID's components + `END:VCALENDAR`, every line copied unchanged.
    pub text: String,
}

fn block_name(line: &str) -> Option<String> {
    let t = line.trim();
    (t.len() > 6 && t[..6].eq_ignore_ascii_case("BEGIN:")).then(|| t[6..].to_ascii_uppercase())
}

fn uid_of(block: &[&str]) -> Option<String> {
    let mut found: Option<String> = None;
    for (i, l) in block.iter().enumerate() {
        let up = l.get(..4).map(str::to_ascii_uppercase);
        if matches!(up.as_deref(), Some("UID:" | "UID;")) {
            let mut v = l[l.find(':')? + 1..]
                .trim_end_matches(['\r', '\n'])
                .to_string();
            for cont in &block[i + 1..] {
                if cont.starts_with(' ') || cont.starts_with('\t') {
                    v.push_str(cont[1..].trim_end_matches(['\r', '\n']));
                } else {
                    break;
                }
            }
            found = Some(v);
            break;
        }
    }
    found
}

/// Split an `.ics` text into per-UID parts.
pub fn split_ics(text: &str) -> Vec<IcsPart> {
    let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let (mut head, mut tz): (Vec<&str>, Vec<&str>) = (vec![], vec![]);
    let mut comps: Vec<(String, Vec<&str>)> = vec![]; // (uid, lines) in order
    let mut i = 0;
    let mut seen_component = false;
    let mut foreign = false; // tasks live in fs-tasks; their text is not part of an event original
    while i < lines.len() {
        let l = lines[i];
        match block_name(l).as_deref() {
            Some("VCALENDAR") | None => {
                let t = l.trim();
                if !t.eq_ignore_ascii_case("END:VCALENDAR") && !seen_component {
                    head.push(l);
                }
                i += 1;
            }
            Some(name) => {
                let end = (i + 1..lines.len())
                    .find(|&j| lines[j].trim().eq_ignore_ascii_case(&format!("END:{name}")))
                    .unwrap_or(lines.len() - 1);
                let block = &lines[i..=end];
                match name {
                    "VTIMEZONE" => tz.extend_from_slice(block),
                    "VEVENT" => {
                        seen_component = true;
                        if let Some(uid) = uid_of(block) {
                            comps.push((uid, block.to_vec()));
                        }
                    }
                    "VTODO" => foreign = true,
                    _ => {}
                }
                seen_component |= name != "VTIMEZONE";
                i = end + 1;
            }
        }
    }
    let mut by_uid: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    let mut order = vec![];
    for (uid, b) in comps {
        if !by_uid.contains_key(&uid) {
            order.push(uid.clone());
        }
        by_uid.entry(uid).or_default().extend(b);
    }
    if order.len() == 1 && !foreign {
        return vec![IcsPart {
            uid: order.remove(0),
            text: text.to_string(),
        }];
    }
    order
        .into_iter()
        .map(|uid| {
            let mut t = String::new();
            head.iter()
                .chain(tz.iter())
                .chain(by_uid[&uid].iter())
                .for_each(|l| t.push_str(l));
            if !t.ends_with('\n') {
                t.push_str(eol);
            }
            t.push_str("END:VCALENDAR");
            t.push_str(eol);
            IcsPart { uid, text: t }
        })
        .collect()
}
