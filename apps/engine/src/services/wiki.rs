//! Wiki filesystem operations — tree, read, write, create, rename, delete.
//! Path-safety enforced by `safe_path`.

use std::fs;
use std::path::{Path, PathBuf};

use modula_types::WikiNode;
use once_cell::sync::Lazy;
use regex::Regex;

use crate::core::error::ApiError;

static SEG_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[^/\x00]{1,200}$").unwrap());

pub fn wiki_root(ws_dir: &Path) -> Result<PathBuf, ApiError> {
    let root = ws_dir.join("wiki");
    if !root.exists() {
        fs::create_dir_all(&root)?;
    }
    Ok(root)
}

fn validate_rel(rel: &str) -> Result<String, ApiError> {
    let rel = rel.trim().trim_start_matches('/').replace('\\', "/");
    if rel.is_empty() {
        return Err(ApiError::BadRequest("path is required".into()));
    }
    for seg in rel.split('/') {
        if seg == "." || seg == ".." {
            return Err(ApiError::BadRequest(format!(
                "path traversal not allowed: {seg:?}"
            )));
        }
        if seg.starts_with('.') {
            return Err(ApiError::BadRequest(
                "path segments may not start with '.'".into(),
            ));
        }
        if !SEG_RE.is_match(seg) {
            return Err(ApiError::BadRequest(format!(
                "invalid path segment: {seg:?}"
            )));
        }
    }
    Ok(rel)
}

fn safe_path(root: &Path, rel: &str) -> Result<PathBuf, ApiError> {
    let rel = validate_rel(rel)?;
    let target = root.join(&rel);
    let resolved = target.canonicalize().unwrap_or(target.clone());
    let root_resolved = root.canonicalize().unwrap_or(root.to_path_buf());
    if !resolved.starts_with(&root_resolved) {
        return Err(ApiError::BadRequest("path escapes wiki root".into()));
    }
    Ok(target)
}

pub fn build_tree(root: &Path) -> Vec<WikiNode> {
    fn walk(dir: &Path, prefix: &str) -> Vec<WikiNode> {
        let mut entries: Vec<_> = fs::read_dir(dir)
            .ok()
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .collect();
        entries.sort_by(|a, b| {
            let af = a.path().is_file();
            let bf = b.path().is_file();
            (af, a.file_name().to_string_lossy().to_lowercase())
                .cmp(&(bf, b.file_name().to_string_lossy().to_lowercase()))
        });
        let mut out = Vec::new();
        for entry in entries {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            let rel = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };
            let path = entry.path();
            if path.is_dir() {
                out.push(WikiNode {
                    name,
                    path: rel.clone(),
                    node_type: "dir".into(),
                    children: walk(&path, &rel),
                });
            } else if path.is_file() {
                out.push(WikiNode {
                    name,
                    path: rel,
                    node_type: "file".into(),
                    children: Vec::new(),
                });
            }
        }
        out
    }
    walk(root, "")
}

pub fn read_file(root: &Path, rel: &str) -> Result<String, ApiError> {
    let target = safe_path(root, rel)?;
    if !target.is_file() {
        return Err(ApiError::NotFound(format!("file not found: {rel}")));
    }
    fs::read_to_string(&target).map_err(|_| ApiError::BadRequest("file is not UTF-8 text".into()))
}

pub fn write_file(root: &Path, rel: &str, content: &str) -> Result<(), ApiError> {
    let target = safe_path(root, rel)?;
    if target.exists() && !target.is_file() {
        return Err(ApiError::BadRequest(format!(
            "path exists and is not a file: {rel}"
        )));
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = target.with_extension(format!(
        "{}.tmp",
        target.extension().and_then(|s| s.to_str()).unwrap_or("md")
    ));
    fs::write(&tmp, content)?;
    fs::rename(&tmp, &target)?;
    Ok(())
}

pub fn create_file(root: &Path, rel: &str, content: &str) -> Result<(), ApiError> {
    let target = safe_path(root, rel)?;
    if target.exists() {
        return Err(ApiError::Conflict(format!("already exists: {rel}")));
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&target, content)?;
    Ok(())
}

pub fn create_folder(root: &Path, rel: &str) -> Result<(), ApiError> {
    let target = safe_path(root, rel)?;
    if target.exists() {
        return Err(ApiError::Conflict(format!("already exists: {rel}")));
    }
    fs::create_dir_all(&target)?;
    Ok(())
}

pub fn rename(root: &Path, src_rel: &str, dst_rel: &str) -> Result<(), ApiError> {
    let src = safe_path(root, src_rel)?;
    let dst = safe_path(root, dst_rel)?;
    if !src.exists() {
        return Err(ApiError::NotFound(format!("not found: {src_rel}")));
    }
    if dst.exists() {
        return Err(ApiError::Conflict(format!("already exists: {dst_rel}")));
    }
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::rename(&src, &dst)?;
    Ok(())
}

pub fn delete(root: &Path, rel: &str) -> Result<(), ApiError> {
    let target = safe_path(root, rel)?;
    if !target.exists() {
        return Err(ApiError::NotFound(format!("not found: {rel}")));
    }
    if target.is_dir() {
        fs::remove_dir_all(&target)?;
    } else {
        fs::remove_file(&target)?;
    }
    Ok(())
}
