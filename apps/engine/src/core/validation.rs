use once_cell::sync::Lazy;
use regex::Regex;

pub static ARG_FLAG_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^--[a-z][a-z0-9-]*$").unwrap());
pub static ARG_KEY_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[a-z][a-z0-9-]*$").unwrap());
