//! Conversions between the thread-domain proto enums (`ThreadKind`,
//! `ThreadVerdict`) and their wire-string forms. The engine stores these as
//! strings ("comment", "ACCEPT") and agents pass the same strings on the CLI,
//! so server handlers and the CLI client share one mapping here rather than
//! each re-deriving it.

use crate::v1::{ThreadKind, ThreadVerdict};

pub fn str_to_kind(s: &str) -> i32 {
    let kind = match s {
        "comment" => ThreadKind::Comment,
        "question" => ThreadKind::Question,
        "verdict" => ThreadKind::Verdict,
        "rework" => ThreadKind::Rework,
        _ => ThreadKind::Unspecified,
    };
    kind as i32
}

pub fn kind_to_str(v: i32) -> &'static str {
    match ThreadKind::try_from(v).unwrap_or(ThreadKind::Unspecified) {
        ThreadKind::Comment => "comment",
        ThreadKind::Question => "question",
        ThreadKind::Verdict => "verdict",
        ThreadKind::Rework => "rework",
        ThreadKind::Unspecified => "",
    }
}

/// `None` for any string that isn't one of the four canonical verdicts (the
/// caller treats that as "no verdict").
pub fn str_to_verdict(s: &str) -> Option<i32> {
    let verdict = match s {
        "ACCEPT" => ThreadVerdict::Accept,
        "REQUEST_CHANGES" => ThreadVerdict::RequestChanges,
        "APPROVE" => ThreadVerdict::Approve,
        "KICK_BACK" => ThreadVerdict::KickBack,
        _ => return None,
    };
    Some(verdict as i32)
}

/// `None` for `UNSPECIFIED` / unknown, so an entry without a verdict renders
/// nothing rather than an empty field.
pub fn verdict_to_str(v: i32) -> Option<&'static str> {
    match ThreadVerdict::try_from(v).unwrap_or(ThreadVerdict::Unspecified) {
        ThreadVerdict::Accept => Some("ACCEPT"),
        ThreadVerdict::RequestChanges => Some("REQUEST_CHANGES"),
        ThreadVerdict::Approve => Some("APPROVE"),
        ThreadVerdict::KickBack => Some("KICK_BACK"),
        ThreadVerdict::Unspecified => None,
    }
}
