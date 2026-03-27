use mlua::prelude::*;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

struct StatusCache {
    map: HashMap<String, u64>,
    order: VecDeque<String>,
}

impl StatusCache {
    const MAX: usize = 2_000;

    fn get(&self, root: &str) -> Option<u64> {
        self.map.get(root).copied()
    }

    fn insert(&mut self, root: String, signature: u64) {
        if !self.map.contains_key(&root) {
            self.order.push_back(root.clone());
            if self.order.len() > Self::MAX {
                if let Some(evicted) = self.order.pop_front() {
                    self.map.remove(&evicted);
                }
            }
        }
        self.map.insert(root, signature);
    }

    fn clear(&mut self) {
        self.map.clear();
        self.order.clear();
        self.map.shrink_to_fit();
        self.order.shrink_to_fit();
    }
}

static STATUS_CACHE: Lazy<Mutex<StatusCache>> = Lazy::new(|| {
    Mutex::new(StatusCache {
        map: HashMap::new(),
        order: VecDeque::new(),
    })
});

// ── Async repo state ──────────────────────────────────────────────────────────

#[derive(Clone)]
struct FileEntry {
    root: String,
    rel: String,
    path: String,
    old_rel: Option<String>,
    index: String,
    worktree: String,
    code: String,
    kind: &'static str,
}

struct RepoState {
    branch: String,
    ahead: i64,
    behind: i64,
    detached: bool,
    dirty: bool,
    refreshing: bool,
    last_refresh: f64,
    error: Option<String>,
    ordered: Vec<FileEntry>,
    /// Maps normalized file path → index in `ordered`.
    files_by_path: HashMap<String, usize>,
}

impl Default for RepoState {
    fn default() -> Self {
        Self {
            branch: String::new(),
            ahead: 0,
            behind: 0,
            detached: false,
            dirty: false,
            refreshing: false,
            last_refresh: 0.0,
            error: None,
            ordered: Vec::new(),
            files_by_path: HashMap::new(),
        }
    }
}

enum RefreshOutcome {
    Success {
        branch: String,
        ahead: i64,
        behind: i64,
        detached: bool,
        ordered: Vec<FileEntry>,
    },
    Failure(String),
}

struct CommandResult {
    ok: bool,
    stdout: String,
    stderr: String,
}

static REPOS: Lazy<Mutex<HashMap<String, RepoState>>> = Lazy::new(|| Mutex::new(HashMap::new()));
static PATH_ROOTS: Lazy<Mutex<HashMap<String, Option<String>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
static PENDING: Lazy<Mutex<VecDeque<(String, RefreshOutcome)>>> =
    Lazy::new(|| Mutex::new(VecDeque::new()));
static COMMANDS: Lazy<Mutex<HashMap<u64, Option<CommandResult>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
static NEXT_HANDLE: AtomicU64 = AtomicU64::new(1);

fn normalize(path: &str) -> String {
    path.replace('\\', "/")
}

/// Seconds elapsed since the first call (monotonic clock, matching `system.get_time()`).
fn monotonic_secs() -> f64 {
    static START: Lazy<std::time::Instant> = Lazy::new(std::time::Instant::now);
    START.elapsed().as_secs_f64()
}

/// Discovers the git root for `path`, with per-directory caching.
fn get_or_discover_root(path: &str) -> Option<String> {
    if path.is_empty() {
        return None;
    }
    let key = normalize(&start_dir(path).to_string_lossy());
    {
        let cache = PATH_ROOTS.lock();
        if let Some(cached) = cache.get(&key) {
            return cached.clone();
        }
    }
    let root = discover_repo(path);
    PATH_ROOTS.lock().insert(key, root.clone());
    root
}

/// Parses `git status --branch --porcelain=v1` output into a `RefreshOutcome`.
fn parse_status_raw(root: &str, stdout: &str, stderr: &str, success: bool) -> RefreshOutcome {
    if !success {
        return RefreshOutcome::Failure(if stderr.trim().is_empty() {
            "git status failed".to_string()
        } else {
            stderr.trim().to_string()
        });
    }
    let mut branch = String::new();
    let mut ahead = 0i64;
    let mut behind = 0i64;
    let mut detached = false;
    let mut entries: Vec<FileEntry> = Vec::new();
    for line in stdout.lines() {
        if let Some(head) = line.strip_prefix("## ") {
            let (b, a, be, d) = parse_branch(head);
            branch = b;
            ahead = a;
            behind = be;
            detached = d;
        } else if !line.starts_with("!!") && line.len() >= 4 {
            let mut rel = line[3..].to_string();
            let mut old_rel: Option<String> = None;
            if let Some((old, new)) = rel.split_once(" -> ") {
                old_rel = Some(old.to_string());
                rel = new.to_string();
            }
            let abs = normalize(&format!("{root}/{rel}"));
            let index = line.chars().next().unwrap_or(' ');
            let worktree = line.chars().nth(1).unwrap_or(' ');
            let code = line[0..2].to_string();
            let kind = classify(&code, index, worktree);
            entries.push(FileEntry {
                root: root.to_string(),
                rel,
                path: abs,
                old_rel,
                index: index.to_string(),
                worktree: worktree.to_string(),
                code,
                kind,
            });
        }
    }
    entries.sort_by(|a, b| {
        if a.kind != b.kind {
            a.kind.cmp(b.kind)
        } else {
            a.rel.cmp(&b.rel)
        }
    });
    RefreshOutcome::Success {
        branch,
        ahead,
        behind,
        detached,
        ordered: entries,
    }
}

/// Spawns a background refresh thread for `root` if none is already running.
fn start_refresh_if_idle(root: String) {
    let should_start = {
        let mut repos = REPOS.lock();
        let state = repos.entry(root.clone()).or_default();
        if state.refreshing {
            false
        } else {
            state.refreshing = true;
            true
        }
    };
    if !should_start {
        return;
    }
    std::thread::spawn(move || {
        let out = Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(["status", "--branch", "--porcelain=v1"])
            .output();
        let outcome = match out {
            Ok(o) => {
                let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                let stderr = String::from_utf8_lossy(&o.stderr).to_string();
                parse_status_raw(&root, &stdout, &stderr, o.status.success())
            }
            Err(e) => RefreshOutcome::Failure(e.to_string()),
        };
        PENDING.lock().push_back((root, outcome));
    });
}

/// Converts a `FileEntry` to a Lua table matching the status.lua schema.
fn entry_to_table(lua: &Lua, e: &FileEntry) -> LuaResult<LuaTable> {
    let t = lua.create_table()?;
    t.set("root", e.root.as_str())?;
    t.set("rel", e.rel.as_str())?;
    t.set("path", e.path.as_str())?;
    match &e.old_rel {
        Some(old) => t.set("old_rel", old.as_str())?,
        None => t.set("old_rel", LuaValue::Nil)?,
    }
    t.set("index", e.index.as_str())?;
    t.set("worktree", e.worktree.as_str())?;
    t.set("code", e.code.as_str())?;
    t.set("kind", e.kind)?;
    Ok(t)
}

fn start_dir(path: &str) -> PathBuf {
    let path = Path::new(path);
    if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent().unwrap_or(path).to_path_buf()
    }
}

fn discover_repo(path: &str) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(start_dir(path))
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let root = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if root.is_empty() {
        None
    } else {
        Some(normalize(&root))
    }
}

fn parse_branch(header: &str) -> (String, i64, i64, bool) {
    fn parse_counter(header: &str, label: &str) -> i64 {
        header
            .split(label)
            .nth(1)
            .and_then(|tail| {
                let digits: String = tail
                    .chars()
                    .skip_while(|ch| !ch.is_ascii_digit())
                    .take_while(|ch| ch.is_ascii_digit())
                    .collect();
                if digits.is_empty() {
                    None
                } else {
                    digits.parse::<i64>().ok()
                }
            })
            .unwrap_or(0)
    }

    let mut branch = header.to_string();
    let ahead = parse_counter(header, "ahead");
    let behind = parse_counter(header, "behind");
    let detached = header.starts_with("HEAD");
    branch = branch
        .split(" [")
        .next()
        .unwrap_or(&branch)
        .split("...")
        .next()
        .unwrap_or(&branch)
        .to_string();
    if branch == "HEAD (no branch)" || branch.starts_with("HEAD (detached") {
        branch = "detached".to_string();
    }
    let is_detached = detached || branch == "detached";
    (branch, ahead, behind, is_detached)
}

fn classify(code: &str, index: char, worktree: char) -> &'static str {
    if code == "??" {
        "untracked"
    } else if index == 'U' || worktree == 'U' {
        "conflict"
    } else if index != ' ' && index != '?' {
        "staged"
    } else if worktree != ' ' {
        "changed"
    } else {
        "unknown"
    }
}

fn status_table(lua: &Lua, root: &str) -> LuaResult<LuaTable> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["status", "--branch", "--porcelain=v1"])
        .output()
        .map_err(|e| LuaError::RuntimeError(e.to_string()))?;
    let repo = lua.create_table()?;
    repo.set("root", root)?;
    repo.set("branch", "")?;
    repo.set("ahead", 0)?;
    repo.set("behind", 0)?;
    repo.set("detached", false)?;
    repo.set("dirty", false)?;
    repo.set("error", LuaValue::Nil)?;
    let files = lua.create_table()?;
    let ordered = lua.create_table()?;
    repo.set("files", files.clone())?;
    repo.set("ordered", ordered.clone())?;

    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        repo.set(
            "error",
            if err.is_empty() {
                "git status failed"
            } else {
                &err
            },
        )?;
        return Ok(repo);
    }

    let mut entries = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        if let Some(head) = line.strip_prefix("## ") {
            let (branch, ahead, behind, detached) = parse_branch(head);
            repo.set("branch", branch)?;
            repo.set("ahead", ahead)?;
            repo.set("behind", behind)?;
            repo.set("detached", detached)?;
        } else if !line.starts_with("!!") && line.len() >= 4 {
            let mut rel = line[3..].to_string();
            let mut old_rel = LuaValue::Nil;
            if let Some((old, new)) = rel.split_once(" -> ") {
                old_rel = LuaValue::String(lua.create_string(old)?);
                rel = new.to_string();
            }
            let abs = normalize(&format!("{root}/{rel}"));
            let index = line.chars().next().unwrap_or(' ');
            let worktree = line.chars().nth(1).unwrap_or(' ');
            let code = &line[0..2];
            let kind = classify(code, index, worktree);
            let entry = lua.create_table()?;
            entry.set("root", root)?;
            entry.set("rel", rel.as_str())?;
            entry.set("path", abs.as_str())?;
            entry.set("old_rel", old_rel)?;
            entry.set("index", index.to_string())?;
            entry.set("worktree", worktree.to_string())?;
            entry.set("code", code)?;
            entry.set("kind", kind)?;
            files.set(abs.as_str(), entry.clone())?;
            entries.push((kind.to_string(), rel, entry));
        }
    }
    entries.sort_by(|a, b| {
        if a.0 != b.0 {
            a.0.cmp(&b.0)
        } else {
            a.1.cmp(&b.1)
        }
    });
    repo.set("dirty", !entries.is_empty())?;
    for (idx, (_, _, entry)) in entries.into_iter().enumerate() {
        ordered.raw_set((idx + 1) as i64, entry)?;
    }
    Ok(repo)
}

fn status_signature(status: i32, stdout: &[u8], stderr: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    status.hash(&mut hasher);
    stdout.hash(&mut hasher);
    stderr.hash(&mut hasher);
    hasher.finish()
}

fn status_cached(lua: &Lua, root: &str) -> LuaResult<LuaTable> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["status", "--branch", "--porcelain=v1"])
        .output()
        .map_err(|e| LuaError::RuntimeError(e.to_string()))?;
    let signature = status_signature(out.status.code().unwrap_or(-1), &out.stdout, &out.stderr);
    let changed = {
        let mut cache = STATUS_CACHE.lock();
        let changed = cache.get(root) != Some(signature);
        cache.insert(root.to_string(), signature);
        changed
    };
    let repo = status_table(lua, root)?;
    repo.set("changed", changed)?;
    repo.set("signature", signature as i64)?;
    Ok(repo)
}

fn list_branches(lua: &Lua, root: &str) -> LuaResult<LuaTable> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["branch", "--all", "--format=%(refname:short)"])
        .output()
        .map_err(|e| LuaError::RuntimeError(e.to_string()))?;
    if !out.status.success() {
        return Err(LuaError::RuntimeError(
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
        ));
    }
    let mut branches = HashSet::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        if !line.trim().is_empty() {
            branches.insert(line.trim().to_string());
        }
    }
    let mut list: Vec<_> = branches.into_iter().collect();
    list.sort();
    let table = lua.create_table()?;
    for (idx, branch) in list.into_iter().enumerate() {
        table.raw_set((idx + 1) as i64, branch)?;
    }
    Ok(table)
}

pub fn make_module(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;
    module.set(
        "discover",
        lua.create_function(|lua, path: String| match discover_repo(&path) {
            Some(root) => Ok(LuaValue::String(lua.create_string(&root)?)),
            None => Ok(LuaValue::Nil),
        })?,
    )?;
    module.set(
        "status",
        lua.create_function(|lua, path: String| {
            let root = discover_repo(&path)
                .ok_or_else(|| LuaError::RuntimeError("Not inside a Git repository".to_string()))?;
            status_table(lua, &root)
        })?,
    )?;
    module.set(
        "status_cached",
        lua.create_function(|lua, path: String| {
            let root = discover_repo(&path)
                .ok_or_else(|| LuaError::RuntimeError("Not inside a Git repository".to_string()))?;
            status_cached(lua, &root)
        })?,
    )?;
    module.set(
        "list_branches",
        lua.create_function(|lua, path: String| {
            let root = discover_repo(&path)
                .ok_or_else(|| LuaError::RuntimeError("Not inside a Git repository".to_string()))?;
            list_branches(lua, &root)
        })?,
    )?;
    module.set(
        "clear_cache",
        lua.create_function(|_, ()| {
            STATUS_CACHE.lock().clear();
            PATH_ROOTS.lock().clear();
            Ok(true)
        })?,
    )?;

    // ── Async state management ─────────────────────────────────────────────────

    module.set(
        "get_root",
        lua.create_function(|lua, path: Option<String>| -> LuaResult<LuaValue> {
            let path = path.unwrap_or_default();
            match get_or_discover_root(&path) {
                Some(root) => Ok(LuaValue::String(lua.create_string(&root)?)),
                None => Ok(LuaValue::Nil),
            }
        })?,
    )?;

    module.set(
        "get_state",
        lua.create_function(|lua, root: String| -> LuaResult<LuaValue> {
            // Clone data before releasing lock to minimise lock duration.
            let data = {
                let repos = REPOS.lock();
                repos.get(&root).map(|s| {
                    (
                        s.branch.clone(),
                        s.ahead,
                        s.behind,
                        s.detached,
                        s.dirty,
                        s.refreshing,
                        s.last_refresh,
                        s.error.clone(),
                        s.ordered.clone(),
                    )
                })
            };
            let Some((
                branch,
                ahead,
                behind,
                detached,
                dirty,
                refreshing,
                last_refresh,
                error,
                ordered,
            )) = data
            else {
                return Ok(LuaValue::Nil);
            };
            let t = lua.create_table()?;
            t.set("root", root.as_str())?;
            t.set("branch", branch.as_str())?;
            t.set("ahead", ahead)?;
            t.set("behind", behind)?;
            t.set("detached", detached)?;
            t.set("dirty", dirty)?;
            t.set("refreshing", refreshing)?;
            t.set("last_refresh", last_refresh)?;
            match error.as_deref() {
                Some(e) => t.set("error", e)?,
                None => t.set("error", LuaValue::Nil)?,
            }
            let ordered_tbl = lua.create_table()?;
            for (i, e) in ordered.iter().enumerate() {
                ordered_tbl.raw_set((i + 1) as i64, entry_to_table(lua, e)?)?;
            }
            t.set("ordered", ordered_tbl)?;
            let files_tbl = lua.create_table()?;
            for e in &ordered {
                files_tbl.set(e.path.as_str(), entry_to_table(lua, e)?)?;
            }
            t.set("files", files_tbl)?;
            Ok(LuaValue::Table(t))
        })?,
    )?;

    module.set(
        "get_file_status",
        lua.create_function(|lua, path: String| -> LuaResult<LuaValue> {
            let norm = normalize(&path);
            let root = match get_or_discover_root(&path) {
                Some(r) => r,
                None => return Ok(LuaValue::Nil),
            };
            let entry = {
                let repos = REPOS.lock();
                repos.get(&root).and_then(|s| {
                    s.files_by_path
                        .get(&norm)
                        .and_then(|&i| s.ordered.get(i))
                        .cloned()
                })
            };
            match entry {
                Some(e) => Ok(LuaValue::Table(entry_to_table(lua, &e)?)),
                None => Ok(LuaValue::Nil),
            }
        })?,
    )?;

    module.set(
        "maybe_refresh",
        lua.create_function(|_, (root, force, interval): (String, bool, f64)| {
            let should_refresh = {
                let repos = REPOS.lock();
                if let Some(s) = repos.get(&root) {
                    if s.refreshing {
                        false
                    } else if force {
                        true
                    } else if s.last_refresh > 0.0 {
                        monotonic_secs() - s.last_refresh >= interval
                    } else {
                        true
                    }
                } else {
                    true
                }
            };
            if should_refresh {
                start_refresh_if_idle(root);
            }
            Ok(())
        })?,
    )?;

    module.set(
        "start_refresh",
        lua.create_function(|_, root: String| {
            start_refresh_if_idle(root);
            Ok(())
        })?,
    )?;

    module.set(
        "poll_updates",
        lua.create_function(|lua, ()| -> LuaResult<LuaValue> {
            let items: Vec<(String, RefreshOutcome)> = {
                let mut pending = PENDING.lock();
                pending.drain(..).collect()
            };
            if items.is_empty() {
                return Ok(LuaValue::Nil);
            }
            let updated: Vec<String> = items.iter().map(|(r, _)| r.clone()).collect();
            {
                let mut repos = REPOS.lock();
                for (root, outcome) in items {
                    let s = repos.entry(root).or_default();
                    s.refreshing = false;
                    s.last_refresh = monotonic_secs();
                    match outcome {
                        RefreshOutcome::Success {
                            branch,
                            ahead,
                            behind,
                            detached,
                            ordered,
                        } => {
                            s.files_by_path.clear();
                            for (i, e) in ordered.iter().enumerate() {
                                s.files_by_path.insert(e.path.clone(), i);
                            }
                            s.ordered = ordered;
                            s.branch = branch;
                            s.ahead = ahead;
                            s.behind = behind;
                            s.detached = detached;
                            s.dirty = !s.ordered.is_empty();
                            s.error = None;
                        }
                        RefreshOutcome::Failure(err) => {
                            s.error = Some(err);
                        }
                    }
                }
            }
            let tbl = lua.create_table()?;
            for (i, root) in updated.iter().enumerate() {
                tbl.raw_set((i + 1) as i64, root.as_str())?;
            }
            Ok(LuaValue::Table(tbl))
        })?,
    )?;

    module.set(
        "start_command",
        lua.create_function(|_, (root, args): (String, Vec<String>)| -> LuaResult<u64> {
            let handle = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
            COMMANDS.lock().insert(handle, None);
            std::thread::spawn(move || {
                let mut cmd = Command::new("git");
                cmd.arg("-C").arg(&root);
                for arg in &args {
                    cmd.arg(arg);
                }
                let result = match cmd.output() {
                    Ok(o) => CommandResult {
                        ok: o.status.success(),
                        stdout: String::from_utf8_lossy(&o.stdout).to_string(),
                        stderr: String::from_utf8_lossy(&o.stderr).trim().to_string(),
                    },
                    Err(e) => CommandResult {
                        ok: false,
                        stdout: String::new(),
                        stderr: e.to_string(),
                    },
                };
                if let Some(slot) = COMMANDS.lock().get_mut(&handle) {
                    *slot = Some(result);
                }
            });
            Ok(handle)
        })?,
    )?;

    module.set(
        "check_command",
        lua.create_function(|lua, handle: u64| -> LuaResult<LuaValue> {
            let result = {
                let mut commands = COMMANDS.lock();
                match commands.get(&handle) {
                    None | Some(None) => return Ok(LuaValue::Nil),
                    Some(Some(_)) => commands.remove(&handle).unwrap().unwrap(),
                }
            };
            let t = lua.create_table()?;
            t.raw_set(1, result.ok)?;
            t.raw_set(2, result.stdout.as_str())?;
            t.raw_set(3, result.stderr.as_str())?;
            Ok(LuaValue::Table(t))
        })?,
    )?;

    Ok(module)
}

#[cfg(test)]
mod tests {
    use super::{classify, parse_branch};

    #[test]
    fn parse_branch_handles_tracking_counters() {
        let (branch, ahead, behind, detached) =
            parse_branch("main...origin/main [ahead 2, behind 1]");
        assert_eq!(branch, "main");
        assert_eq!(ahead, 2);
        assert_eq!(behind, 1);
        assert!(!detached);
    }

    #[test]
    fn parse_branch_handles_detached_head() {
        let (branch, ahead, behind, detached) = parse_branch("HEAD (detached at 1234567)");
        assert_eq!(branch, "detached");
        assert_eq!(ahead, 0);
        assert_eq!(behind, 0);
        assert!(detached);
    }

    #[test]
    fn classify_distinguishes_untracked_conflict_staged_and_changed() {
        assert_eq!(classify("??", '?', '?'), "untracked");
        assert_eq!(classify("UU", 'U', 'U'), "conflict");
        assert_eq!(classify("M ", 'M', ' '), "staged");
        assert_eq!(classify(" M", ' ', 'M'), "changed");
        assert_eq!(classify("  ", ' ', ' '), "unknown");
    }
}
