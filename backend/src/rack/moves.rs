//! Moving devices in the layout: which moves a request may make (`plan`),
//! and the file's document with exactly those moves made (`rewrite`).
//!
//! Only this changes in the document: a moved device's `u`, its
//! `u_confirmed` (true: somebody placed it), the rack whose `devices` holds
//! it, the top-level `version`, and one `history` entry for each device.
//! Everything else stays as written, in the order written.

use serde::Deserialize;

use super::json::Json;
use super::layout::Layout;

/// Moves in one save at most.
pub(super) const MAX_MOVES: usize = 64;
/// History entries kept; the oldest go first.
pub(super) const MAX_HISTORY: usize = 500;

/// One requested move: this device's top slot becomes `u` of `rack`.
#[derive(Deserialize, Clone, Debug)]
pub(crate) struct Move {
    pub id: String,
    pub rack: String,
    pub u: u32,
}

/// A move that changes something.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Moved {
    pub id: String,
    pub from_rack: String,
    /// `None`: it had no slot yet.
    pub from_u: Option<u32>,
    pub to_rack: String,
    pub to_u: u32,
}

/// Why the moves cannot be saved: a word for the page, and the sentence.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Refusal {
    /// `too_many`, `duplicate`, `unknown_device`, `unknown_rack`,
    /// `out_of_rack`, `overlap`, `nothing_to_move`.
    pub reason: &'static str,
    pub detail: String,
}

fn refuse<T>(reason: &'static str, detail: String) -> Result<T, Refusal> {
    Err(Refusal { reason, detail })
}

/// The moves that change something, checked against the layout as it is:
/// each device and rack exists, each device fits its new rack, and with
/// ALL the moves made no moved device shares a slot with another.
pub(super) fn plan(layout: &Layout, moves: &[Move]) -> Result<Vec<Moved>, Refusal> {
    if moves.len() > MAX_MOVES {
        return refuse("too_many", format!("{} moves given; at most {MAX_MOVES} in one save", moves.len()));
    }
    let mut moved: Vec<Moved> = Vec::new();
    // Where everything ends up: (rack, device, lowest slot, top slot).
    let mut places: Vec<(String, String, u32, u32)> = Vec::new();
    for rack in &layout.racks {
        for d in &rack.devices {
            if let Some(u) = d.u {
                places.push((rack.id.clone(), d.id.clone(), u + 1 - d.h, u));
            }
        }
    }
    for (i, m) in moves.iter().enumerate() {
        if moves[..i].iter().any(|earlier| earlier.id == m.id) {
            return refuse("duplicate", format!("{}: moved twice in one save", m.id));
        }
        let found = layout.racks.iter().find_map(|r| r.devices.iter().find(|d| d.id == m.id).map(|d| (r, d)));
        let Some((from, device)) = found else {
            return refuse("unknown_device", format!("{}: no such device in the layout", shown(&m.id)));
        };
        let Some(to) = layout.racks.iter().find(|r| r.id == m.rack) else {
            return refuse("unknown_rack", format!("{}: no such rack in the layout", shown(&m.rack)));
        };
        // `u` is the top slot: the device reaches down to u - h + 1.
        if m.u == 0 || m.u > to.units || m.u < device.h {
            return refuse("out_of_rack", format!("{}: {}U high with its top at U{} does not fit U1–U{} of {}", device.id, device.h, m.u, to.units, to.id));
        }
        if from.id == to.id && device.u == Some(m.u) {
            continue;
        }
        places.retain(|(_, id, _, _)| id != &device.id);
        places.push((to.id.clone(), device.id.clone(), m.u + 1 - device.h, m.u));
        moved.push(Moved { id: device.id.clone(), from_rack: from.id.clone(), from_u: device.u, to_rack: to.id.clone(), to_u: m.u });
    }
    if moved.is_empty() {
        return refuse("nothing_to_move", "no device changes place: nothing to save".into());
    }
    for m in &moved {
        let Some((rack, _, low, top)) = places.iter().find(|(_, id, _, _)| id == &m.id) else { continue };
        if let Some((_, other, _, _)) = places.iter().find(|(r, id, l, t)| r == rack && id != &m.id && l <= top && low <= t) {
            let slots = if low == top { format!("U{top}") } else { format!("U{low}–U{top}") };
            return refuse("overlap", format!("{}: {slots} of {rack} is taken by {other}", m.id));
        }
    }
    Ok(moved)
}

/// A name from the request, cut short for a message.
fn shown(name: &str) -> String {
    name.chars().filter(|c| !c.is_control()).take(64).collect()
}

fn rack_at<'a>(doc: &'a mut Json, id: &str) -> Result<&'a mut Json, String> {
    doc.get_mut("racks")
        .and_then(Json::as_array_mut)
        .and_then(|racks| racks.iter_mut().find(|r| r.get("id").and_then(Json::as_str) == Some(id)))
        .ok_or_else(|| format!("rack {id} is not in the document"))
}

/// Make the moves in the document and add them to its history. `at` is the
/// time (UTC), `by` who saved. The version is set with `set_version`.
pub(super) fn rewrite(doc: &mut Json, moved: &[Moved], at: &str, by: &str) -> Result<(), String> {
    for m in moved {
        let from = rack_at(doc, &m.from_rack)?;
        let devices = from.get_mut("devices").and_then(Json::as_array_mut).ok_or_else(|| format!("rack {} has no devices", m.from_rack))?;
        let index = devices
            .iter()
            .position(|d| d.get("id").and_then(Json::as_str) == Some(m.id.as_str()))
            .ok_or_else(|| format!("device {} is not in rack {}", m.id, m.from_rack))?;
        let device = &mut devices[index];
        if device.entries().is_none() {
            return Err(format!("device {} is not an object", m.id));
        }
        device.set("u", Json::number(m.to_u), &["type", "label", "id"]);
        device.set("u_confirmed", Json::Bool(true), &["h", "u"]);
        if m.from_rack != m.to_rack {
            let device = devices.remove(index);
            let to = rack_at(doc, &m.to_rack)?;
            if to.get("devices").is_none() {
                to.set("devices", Json::Array(Vec::new()), &["units"]);
            }
            to.get_mut("devices").and_then(Json::as_array_mut).ok_or_else(|| format!("rack {}: devices is not a list", m.to_rack))?.push(device);
        }
    }
    let place = |rack: &str, u: Option<u32>| Json::Object(vec![("rack".into(), Json::text(rack)), ("u".into(), u.map_or(Json::Null, Json::number))]);
    if doc.get("history").is_none() {
        doc.set("history", Json::Array(Vec::new()), &[]);
    }
    let history = doc.get_mut("history").and_then(Json::as_array_mut).ok_or("history is not a list")?;
    for m in moved {
        history.push(Json::Object(vec![
            ("at".into(), Json::text(at)),
            ("by".into(), Json::text(by)),
            ("id".into(), Json::text(&m.id)),
            ("from".into(), place(&m.from_rack, m.from_u)),
            ("to".into(), place(&m.to_rack, Some(m.to_u))),
        ]));
    }
    if history.len() > MAX_HISTORY {
        history.drain(..history.len() - MAX_HISTORY);
    }
    Ok(())
}

/// The document's `version`; a document without one gets it first.
pub(super) fn set_version(doc: &mut Json, version: &str) {
    if doc.get("version").is_some() {
        doc.set("version", Json::text(version), &[]);
    } else if let Some(entries) = doc.entries_mut() {
        entries.insert(0, ("version".into(), Json::text(version)));
    }
}
