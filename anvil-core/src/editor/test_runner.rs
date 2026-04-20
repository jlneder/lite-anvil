//! Test runner: detect the project's test framework, build run-all /
//! run-file / run-single commands, and discover individual test
//! definitions in the active document for inline "Run test" badges.
//!
//! Detection logic ported verbatim from the 1.5.5 Lua plugin
//! (`forge-core/src/editor/plugins/test_runner.rs`, removed in 2.0.0
//! with the rest of the Lua bridge).

use std::path::Path;

/// Detected test framework kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunnerKind {
    Cargo,
    NodeVitest,
    NodeJest,
    NodeNpm,
    Pytest,
    Unittest,
    Go,
    Dotnet,
    Sbt,
    Gradle,
    Maven,
    PhpUnit,
    Make,
}

/// A detected test runner: kind, project root, and the run-all command.
#[derive(Debug, Clone)]
pub struct Runner {
    pub kind: RunnerKind,
    pub project_path: String,
    pub run_all: String,
}

/// Detect the project's test framework given the project root path.
/// Returns `None` if `project_path` is empty or has no recognised
/// marker file. Use [`detect_runner_for_file`] to walk up from a file
/// path instead, or [`detect_runner_with_fallback`] for the common
/// "try project root, otherwise walk up from the active file" case.
pub fn detect_runner(project_path: &str) -> Option<Runner> {
    if project_path.is_empty() {
        return None;
    }
    detect_runner_at(Path::new(project_path))
}

/// Walk up from the given file's directory looking for a marker file.
/// Returns the first ancestor (closest to the file) that matches.
pub fn detect_runner_for_file(file_path: &str) -> Option<Runner> {
    if file_path.is_empty() {
        return None;
    }
    let parent = Path::new(file_path).parent()?;
    for dir in parent.ancestors() {
        if let Some(r) = detect_runner_at(dir) {
            return Some(r);
        }
    }
    None
}

/// Convenience: try `project_root` first; on miss, walk up from `active_file`.
pub fn detect_runner_with_fallback(project_root: &str, active_file: &str) -> Option<Runner> {
    detect_runner(project_root).or_else(|| detect_runner_for_file(active_file))
}

/// Internal: detect at a single directory without recursing.
fn detect_runner_at(root: &Path) -> Option<Runner> {
    let project_path = root.to_string_lossy().into_owned();
    let exists = |name: &str| root.join(name).exists();

    if exists("Cargo.toml") {
        return Some(Runner {
            kind: RunnerKind::Cargo,
            project_path: project_path.clone(),
            run_all: "cargo test".into(),
        });
    }
    if exists("package.json") {
        let (kind, run_all) = if exists("node_modules/.bin/vitest") {
            (RunnerKind::NodeVitest, "npx vitest run".into())
        } else if exists("node_modules/.bin/jest") {
            (RunnerKind::NodeJest, "npx jest".into())
        } else {
            (RunnerKind::NodeNpm, "npm test".into())
        };
        return Some(Runner {
            kind,
            project_path: project_path.clone(),
            run_all,
        });
    }
    if exists("pytest.ini") || exists("conftest.py") {
        return Some(Runner {
            kind: RunnerKind::Pytest,
            project_path: project_path.clone(),
            run_all: "python -m pytest -v".into(),
        });
    }
    if exists("pyproject.toml") || exists("setup.py") || exists("setup.cfg") {
        let has_pytest = if exists("pyproject.toml") {
            std::fs::read_to_string(root.join("pyproject.toml"))
                .map(|s| s.contains("[tool.pytest") || s.contains("pytest"))
                .unwrap_or(false)
        } else {
            false
        };
        if has_pytest {
            return Some(Runner {
                kind: RunnerKind::Pytest,
                project_path: project_path.clone(),
                run_all: "python -m pytest -v".into(),
            });
        } else {
            return Some(Runner {
                kind: RunnerKind::Unittest,
                project_path: project_path.clone(),
                run_all: "python -m unittest discover -v".into(),
            });
        }
    }
    if exists("go.mod") {
        return Some(Runner {
            kind: RunnerKind::Go,
            project_path: project_path.clone(),
            run_all: "go test ./...".into(),
        });
    }
    if has_extension(root, "sln") || has_extension(root, "csproj") || has_extension(root, "fsproj")
    {
        return Some(Runner {
            kind: RunnerKind::Dotnet,
            project_path: project_path.clone(),
            run_all: "dotnet test".into(),
        });
    }
    if exists("build.sbt") {
        return Some(Runner {
            kind: RunnerKind::Sbt,
            project_path: project_path.clone(),
            run_all: "sbt test".into(),
        });
    }
    if exists("build.gradle") || exists("build.gradle.kts") {
        let run_all = if exists("gradlew") {
            "./gradlew test".into()
        } else {
            "gradle test".into()
        };
        return Some(Runner {
            kind: RunnerKind::Gradle,
            project_path: project_path.clone(),
            run_all,
        });
    }
    if exists("pom.xml") {
        let run_all = if exists("mvnw") {
            "./mvnw test".into()
        } else {
            "mvn test".into()
        };
        return Some(Runner {
            kind: RunnerKind::Maven,
            project_path: project_path.clone(),
            run_all,
        });
    }
    if exists("phpunit.xml") || exists("phpunit.xml.dist") {
        let run_all = if exists("vendor/bin/phpunit") {
            "./vendor/bin/phpunit".into()
        } else {
            "phpunit".into()
        };
        return Some(Runner {
            kind: RunnerKind::PhpUnit,
            project_path: project_path.clone(),
            run_all,
        });
    }
    if exists("tests/conftest.py") || exists("test/conftest.py") {
        return Some(Runner {
            kind: RunnerKind::Pytest,
            project_path: project_path.clone(),
            run_all: "python -m pytest -v".into(),
        });
    }
    if exists("Makefile") || exists("makefile") {
        return Some(Runner {
            kind: RunnerKind::Make,
            project_path: project_path.clone(),
            run_all: "make test".into(),
        });
    }
    None
}

/// True if any direct child of `dir` ends with `.{ext}`.
fn has_extension(dir: &Path, ext: &str) -> bool {
    let suffix = format!(".{ext}");
    std::fs::read_dir(dir)
        .map(|it| {
            it.flatten().any(|e| {
                e.file_name()
                    .to_string_lossy()
                    .to_ascii_lowercase()
                    .ends_with(&suffix)
            })
        })
        .unwrap_or(false)
}

/// Derive a Cargo module filter from an absolute file path and project root.
/// `/home/u/proj/src/foo/bar.rs` with root `/home/u/proj` → `Some("foo::bar")`.
/// Returns `None` for `lib.rs` / `main.rs` / paths outside the project.
pub fn cargo_module_filter(file_path: &str, project_path: &str) -> Option<String> {
    let relative = file_path
        .strip_prefix(project_path)?
        .trim_start_matches('/');
    let after_src = if let Some(pos) = relative.find("/src/") {
        &relative[pos + 5..]
    } else if let Some(rest) = relative.strip_prefix("src/") {
        rest
    } else {
        relative
    };
    let stem = after_src.strip_suffix(".rs")?;
    if stem == "lib" || stem == "main" {
        return None;
    }
    let stem = stem.strip_suffix("/mod").unwrap_or(stem);
    if stem.is_empty() {
        return None;
    }
    Some(stem.replace('/', "::"))
}

/// Last path component minus any of the supplied extensions.
fn class_name(file_path: &str, extensions: &[&str]) -> String {
    let name = file_path.rsplit('/').next().unwrap_or(file_path);
    for ext in extensions {
        if let Some(stem) = name.strip_suffix(ext) {
            return stem.to_owned();
        }
    }
    name.to_owned()
}

/// Build the file-scoped test command for a given runner.
/// Returns `None` if the runner has no useful per-file form (caller falls
/// back to the run-all command).
pub fn file_test_command(runner: &Runner, file_path: &str) -> Option<String> {
    let project_path = &runner.project_path;
    match runner.kind {
        RunnerKind::Cargo => {
            let filter = cargo_module_filter(file_path, project_path)?;
            Some(format!("cargo test {filter}"))
        }
        RunnerKind::Pytest => Some(format!("python -m pytest -v {file_path}")),
        RunnerKind::Unittest => {
            let rel = file_path
                .strip_prefix(project_path)?
                .trim_start_matches('/');
            let module = rel.strip_suffix(".py")?.replace('/', ".");
            Some(format!("python -m unittest -v {module}"))
        }
        RunnerKind::Go => {
            let dir = file_path.rsplit_once('/')?.0;
            let rel = dir.strip_prefix(project_path)?.trim_start_matches('/');
            if rel.is_empty() {
                Some("go test -v .".into())
            } else {
                Some(format!("go test -v ./{rel}"))
            }
        }
        RunnerKind::NodeVitest | RunnerKind::NodeJest => {
            Some(format!("{} {}", runner.run_all, file_path))
        }
        RunnerKind::NodeNpm => None,
        RunnerKind::Dotnet => {
            let class = class_name(file_path, &[".cs", ".fs"]);
            Some(format!("dotnet test --filter FullyQualifiedName~{class}"))
        }
        RunnerKind::Sbt => {
            let class = class_name(file_path, &[".scala"]);
            Some(format!("sbt \"testOnly *{class}*\""))
        }
        RunnerKind::Gradle => {
            let class = class_name(file_path, &[".java", ".kt", ".scala"]);
            Some(format!("./gradlew test --tests \"*{class}*\""))
        }
        RunnerKind::Maven => {
            let class = class_name(file_path, &[".java", ".kt", ".scala"]);
            Some(format!("mvn test -Dtest=\"{class}\""))
        }
        RunnerKind::PhpUnit => Some(format!("{} {}", runner.run_all, file_path)),
        RunnerKind::Make => None,
    }
}

/// Build the single-test command for `test_name` in `file_path`.
/// `test_name` is the unqualified function or block name (e.g. `test_foo`,
/// `TestBar`, `it("renders")` → `renders`). Returns `None` if the runner
/// has no single-test form.
pub fn single_test_command(runner: &Runner, file_path: &str, test_name: &str) -> Option<String> {
    match runner.kind {
        RunnerKind::Cargo => match cargo_module_filter(file_path, &runner.project_path) {
            Some(module) => Some(format!("cargo test {module}::{test_name}")),
            None => Some(format!("cargo test {test_name}")),
        },
        RunnerKind::Pytest => Some(format!("python -m pytest -v {file_path}::{test_name}")),
        RunnerKind::Unittest => {
            let rel = file_path
                .strip_prefix(&runner.project_path)?
                .trim_start_matches('/');
            let module = rel.strip_suffix(".py")?.replace('/', ".");
            Some(format!("python -m unittest -v {module}.{test_name}"))
        }
        RunnerKind::Go => {
            let dir = file_path.rsplit_once('/')?.0;
            let rel = dir
                .strip_prefix(&runner.project_path)?
                .trim_start_matches('/');
            let pkg = if rel.is_empty() {
                ".".into()
            } else {
                format!("./{rel}")
            };
            Some(format!("go test -v -run ^{test_name}$ {pkg}"))
        }
        RunnerKind::NodeVitest | RunnerKind::NodeJest => Some(format!(
            "{} {} -t \"{test_name}\"",
            runner.run_all, file_path
        )),
        RunnerKind::NodeNpm => None,
        RunnerKind::Dotnet => {
            let class = class_name(file_path, &[".cs", ".fs"]);
            Some(format!(
                "dotnet test --filter FullyQualifiedName~{class}.{test_name}"
            ))
        }
        RunnerKind::Sbt => {
            let class = class_name(file_path, &[".scala"]);
            Some(format!(
                "sbt \"testOnly *{class}* -- -z \\\"{test_name}\\\"\""
            ))
        }
        RunnerKind::Gradle => {
            let class = class_name(file_path, &[".java", ".kt", ".scala"]);
            Some(format!("./gradlew test --tests \"*{class}.{test_name}\""))
        }
        RunnerKind::Maven => {
            let class = class_name(file_path, &[".java", ".kt", ".scala"]);
            Some(format!("mvn test -Dtest=\"{class}#{test_name}\""))
        }
        RunnerKind::PhpUnit => Some(format!(
            "{} --filter \"{test_name}\" {file_path}",
            runner.run_all
        )),
        RunnerKind::Make => None,
    }
}

/// One discovered test in the active document.
///
/// `line` is 1-based, matching the editor's display. `name` is the
/// unqualified test identifier (function name, or string literal for JS
/// `it(...)`). `attribute_line` points at the `#[test]` attribute (or
/// equivalent prefix) so the renderer can choose where to draw the badge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredTest {
    pub line: usize,
    pub attribute_line: usize,
    pub name: String,
}

/// Scan the document text for test definitions. The language is inferred
/// from the file extension — unknown extensions return an empty vec.
///
/// This is a deliberately small lexical scan: we look for the test
/// markers VS Code's CodeLens equivalents look for, with no full parse.
/// Comments and strings can produce false positives, which is acceptable
/// for a "Run this test" affordance.
pub fn discover_tests(file_path: &str, lines: &[String]) -> Vec<DiscoveredTest> {
    let ext = file_path
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "rs" => discover_rust(lines),
        "py" => discover_python(lines),
        "go" => discover_go(lines),
        "js" | "jsx" | "ts" | "tsx" | "mjs" | "cjs" => discover_js(lines),
        _ => Vec::new(),
    }
}

fn discover_rust(lines: &[String]) -> Vec<DiscoveredTest> {
    let mut out = Vec::new();
    let mut pending_attr_line: Option<usize> = None;
    for (idx, raw) in lines.iter().enumerate() {
        let line_no = idx + 1;
        let trimmed = raw.trim_start();
        if trimmed.starts_with("#[test]")
            || trimmed.starts_with("#[tokio::test")
            || trimmed.starts_with("#[async_std::test")
            || trimmed.starts_with("#[actix_web::test")
            || trimmed.starts_with("#[rstest")
        {
            pending_attr_line = Some(line_no);
            continue;
        }
        if let Some(attr_line) = pending_attr_line
            && let Some(name) = parse_rust_fn_name(trimmed)
        {
            out.push(DiscoveredTest {
                line: line_no,
                attribute_line: attr_line,
                name,
            });
            pending_attr_line = None;
        } else if !trimmed.is_empty() && !trimmed.starts_with("#[") && !trimmed.starts_with("//") {
            // Non-attribute, non-blank line clears the pending attribute.
            pending_attr_line = None;
        }
    }
    out
}

fn parse_rust_fn_name(trimmed: &str) -> Option<String> {
    let after = trimmed
        .strip_prefix("pub ")
        .or_else(|| trimmed.strip_prefix("pub(crate) "))
        .unwrap_or(trimmed);
    let after = after.strip_prefix("async ").unwrap_or(after);
    let rest = after.strip_prefix("fn ")?;
    let name: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    if name.is_empty() { None } else { Some(name) }
}

fn discover_python(lines: &[String]) -> Vec<DiscoveredTest> {
    let mut out = Vec::new();
    for (idx, raw) in lines.iter().enumerate() {
        let line_no = idx + 1;
        let trimmed = raw.trim_start();
        let body = trimmed
            .strip_prefix("async def ")
            .or_else(|| trimmed.strip_prefix("def "));
        if let Some(rest) = body {
            let name: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if name.starts_with("test_") || name == "test" {
                out.push(DiscoveredTest {
                    line: line_no,
                    attribute_line: line_no,
                    name,
                });
            }
        }
    }
    out
}

fn discover_go(lines: &[String]) -> Vec<DiscoveredTest> {
    let mut out = Vec::new();
    for (idx, raw) in lines.iter().enumerate() {
        let line_no = idx + 1;
        let trimmed = raw.trim_start();
        if let Some(rest) = trimmed.strip_prefix("func ") {
            let after_recv = if let Some(stripped) = rest.strip_prefix('(') {
                stripped
                    .find(')')
                    .map(|p| stripped[p + 1..].trim_start())
                    .unwrap_or(rest)
            } else {
                rest
            };
            let name: String = after_recv
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if (name.starts_with("Test")
                || name.starts_with("Benchmark")
                || name.starts_with("Example"))
                && name.len() > 4
            {
                out.push(DiscoveredTest {
                    line: line_no,
                    attribute_line: line_no,
                    name,
                });
            }
        }
    }
    out
}

fn discover_js(lines: &[String]) -> Vec<DiscoveredTest> {
    let mut out = Vec::new();
    for (idx, raw) in lines.iter().enumerate() {
        let line_no = idx + 1;
        let trimmed = raw.trim_start();
        for prefix in [
            "it(",
            "it.only(",
            "it.skip(",
            "test(",
            "test.only(",
            "test.skip(",
            "describe(",
        ] {
            if let Some(rest) = trimmed.strip_prefix(prefix)
                && let Some(name) = parse_js_quoted_arg(rest)
            {
                out.push(DiscoveredTest {
                    line: line_no,
                    attribute_line: line_no,
                    name,
                });
                break;
            }
        }
    }
    out
}

fn parse_js_quoted_arg(rest: &str) -> Option<String> {
    let bytes = rest.as_bytes();
    let quote = match bytes.first()? {
        b'"' | b'\'' | b'`' => bytes[0],
        _ => return None,
    };
    let mut name = String::new();
    let mut i = 1;
    while i < bytes.len() {
        let c = bytes[i];
        if c == quote {
            return Some(name);
        }
        if c == b'\\' && i + 1 < bytes.len() {
            name.push(bytes[i + 1] as char);
            i += 2;
            continue;
        }
        name.push(c as char);
        i += 1;
    }
    None
}

/// Hit-test rect for an inline "Run test" badge, populated each frame
/// during draw and consumed by the next mouse-click handler.
#[derive(Debug, Clone)]
pub struct TestBadgeRegion {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    /// Index into `OpenDoc::discovered_tests`.
    pub test_index: usize,
}

/// Spawn a fresh terminal pane, set its title, and write the test
/// command into it. The terminal panel becomes visible and focused.
/// No-op (with stderr log) if the panel is at the terminal limit or
/// the spawn fails.
#[cfg(any(unix, windows))]
pub(crate) fn launch_in_terminal(
    panel: &mut crate::editor::terminal_panel::TerminalPanel,
    cwd: &str,
    command: &str,
    title: &str,
) {
    if !panel.spawn(cwd) {
        eprintln!("[test_runner] could not spawn terminal (limit reached?)");
        return;
    }
    if let Some(t) = panel.active_terminal() {
        t.title = title.to_string();
        let payload = format!("{command}\n");
        let _ = t.inner.write(payload.as_bytes());
    }
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn launch_in_terminal(
    _panel: &mut crate::editor::terminal_panel::TerminalPanel,
    _cwd: &str,
    _command: &str,
    _title: &str,
) {
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cargo_module_filter_strips_src_and_rs() {
        assert_eq!(
            cargo_module_filter("/proj/src/foo/bar.rs", "/proj"),
            Some("foo::bar".into())
        );
    }

    #[test]
    fn cargo_module_filter_strips_workspace_member() {
        assert_eq!(
            cargo_module_filter("/proj/anvil-core/src/editor/test_runner.rs", "/proj"),
            Some("editor::test_runner".into())
        );
    }

    #[test]
    fn cargo_module_filter_handles_mod_rs() {
        assert_eq!(
            cargo_module_filter("/proj/src/editor/mod.rs", "/proj"),
            Some("editor".into())
        );
    }

    #[test]
    fn cargo_module_filter_skips_main_and_lib() {
        assert_eq!(cargo_module_filter("/proj/src/main.rs", "/proj"), None);
        assert_eq!(cargo_module_filter("/proj/src/lib.rs", "/proj"), None);
    }

    #[test]
    fn discover_rust_finds_test_functions() {
        let lines: Vec<String> = vec![
            "use std::path::Path;".into(),
            "".into(),
            "#[cfg(test)]".into(),
            "mod tests {".into(),
            "    use super::*;".into(),
            "".into(),
            "    #[test]".into(),
            "    fn alpha_works() {".into(),
            "        assert!(true);".into(),
            "    }".into(),
            "".into(),
            "    #[tokio::test]".into(),
            "    async fn bravo_async() {}".into(),
            "}".into(),
        ];
        let found = discover_rust(&lines);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].name, "alpha_works");
        assert_eq!(found[0].line, 8);
        assert_eq!(found[0].attribute_line, 7);
        assert_eq!(found[1].name, "bravo_async");
    }

    #[test]
    fn discover_rust_clears_pending_attr_on_unrelated_code() {
        // A `#[test]` followed by something other than a fn (e.g. a
        // misplaced doc comment that's been edited out) should NOT bind
        // to the next fn ten lines down.
        let lines: Vec<String> = vec![
            "#[test]".into(),
            "let x = 1;".into(), // breaks the binding
            "fn not_a_test() {}".into(),
        ];
        assert!(discover_rust(&lines).is_empty());
    }

    #[test]
    fn discover_python_recognises_test_functions() {
        let lines: Vec<String> = vec![
            "def helper():".into(),
            "    pass".into(),
            "def test_alpha():".into(),
            "    assert True".into(),
            "async def test_bravo():".into(),
            "    pass".into(),
        ];
        let found = discover_python(&lines);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].name, "test_alpha");
        assert_eq!(found[1].name, "test_bravo");
    }

    #[test]
    fn discover_go_recognises_capitalised_tests() {
        let lines: Vec<String> = vec![
            "package x".into(),
            "func TestAlpha(t *testing.T) {}".into(),
            "func helper() {}".into(),
            "func BenchmarkBravo(b *testing.B) {}".into(),
        ];
        let found = discover_go(&lines);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].name, "TestAlpha");
        assert_eq!(found[1].name, "BenchmarkBravo");
    }

    #[test]
    fn discover_js_extracts_quoted_name() {
        let lines: Vec<String> = vec![
            "it(\"renders correctly\", () => {});".into(),
            "test('handles input', () => {});".into(),
            "describe(`group`, () => {});".into(),
        ];
        let found = discover_js(&lines);
        assert_eq!(found.len(), 3);
        assert_eq!(found[0].name, "renders correctly");
        assert_eq!(found[1].name, "handles input");
        assert_eq!(found[2].name, "group");
    }

    #[test]
    fn single_test_command_cargo_includes_module_path() {
        let runner = Runner {
            kind: RunnerKind::Cargo,
            project_path: "/proj".into(),
            run_all: "cargo test".into(),
        };
        assert_eq!(
            single_test_command(&runner, "/proj/src/editor/foo.rs", "test_x"),
            Some("cargo test editor::foo::test_x".into())
        );
    }

    #[test]
    fn single_test_command_pytest_uses_node_id() {
        let runner = Runner {
            kind: RunnerKind::Pytest,
            project_path: "/proj".into(),
            run_all: "python -m pytest -v".into(),
        };
        assert_eq!(
            single_test_command(&runner, "/proj/tests/test_foo.py", "test_alpha"),
            Some("python -m pytest -v /proj/tests/test_foo.py::test_alpha".into())
        );
    }
}
