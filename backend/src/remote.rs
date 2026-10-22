//! What Remote gives NabiMan. NabiMan knows Remote only through the points
//! registered here (`extension_points`); everything else is Remote's own.

mod report;

use crate::extension_points::ExtensionPoints;

pub(crate) fn extension_points() -> ExtensionPoints {
    ExtensionPoints {
        // Share links carry their own secret and are checked in terminal::share.
        public_prefixes: vec!["/api/share/"],
        url_passes: vec![crate::terminal_tickets::url_pass, crate::terminal::sftp::links::url_pass],
        report_parts: vec![("terminal_registry", report::terminals), ("server_inventory", report::servers)],
        // A member's own server is theirs alone, not something to grant others.
        server_exists: Some(|id| crate::remote_servers::find_server(id).map(|s| s.is_some_and(|s| !s.member_owned()))),
    }
}
