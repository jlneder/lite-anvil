use mlua::prelude::*;
use std::cmp::Ordering;
use std::collections::HashSet;

fn compare_ranked(a: &(String, i64), b: &(String, i64)) -> Ordering {
    match b.1.cmp(&a.1) {
        Ordering::Equal => a.0.cmp(&b.0),
        other => other,
    }
}

fn normalize_needle(needle: &str, files: bool) -> String {
    if cfg!(windows) && files {
        needle.replace('/', "\\")
    } else {
        needle.to_string()
    }
}

fn rank_strings_inner(
    items: Vec<String>,
    needle: &str,
    files: bool,
    recents: &[String],
    limit: Option<usize>,
) -> Vec<String> {
    let needle = normalize_needle(needle, files);
    let mut out = Vec::new();
    let mut seen = HashSet::new();

    if needle.is_empty() {
        for recent in recents {
            if seen.insert(recent.clone()) {
                out.push(recent.clone());
            }
        }
    }

    let mut ranked = Vec::new();
    for item in items {
        if let Some(score) = crate::editor::fuzzy_match(&item, &needle, files) {
            ranked.push((item, score));
        }
    }
    ranked.sort_by(compare_ranked);

    for (item, _) in ranked {
        if seen.insert(item.clone()) {
            out.push(item);
            if let Some(limit) = limit {
                if out.len() >= limit {
                    break;
                }
            }
        }
    }

    out
}

pub fn make_module(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;

    module.set(
        "rank_strings",
        lua.create_function(
            |lua,
             (items, needle, files, recents, limit): (
                LuaTable,
                String,
                Option<bool>,
                Option<LuaTable>,
                Option<usize>,
            )| {
                let mut strings = Vec::new();
                for entry in items.sequence_values::<String>() {
                    strings.push(entry?);
                }

                let mut recent_items = Vec::new();
                if let Some(recents) = recents {
                    for entry in recents.sequence_values::<String>() {
                        recent_items.push(entry?);
                    }
                }

                let ranked = rank_strings_inner(
                    strings,
                    &needle,
                    files.unwrap_or(false),
                    &recent_items,
                    limit,
                );
                let out = lua.create_table_with_capacity(ranked.len(), 0)?;
                for (idx, item) in ranked.into_iter().enumerate() {
                    out.raw_set((idx + 1) as i64, item)?;
                }
                Ok(out)
            },
        )?,
    )?;

    module.set(
        "rank_items",
        lua.create_function(
            |lua,
             (items, needle, field, files, recents, limit): (
                LuaTable,
                String,
                Option<String>,
                Option<bool>,
                Option<LuaTable>,
                Option<usize>,
            )| {
                let field = field.unwrap_or_else(|| "text".to_string());
                let files = files.unwrap_or(false);
                let needle = normalize_needle(&needle, files);

                let mut recent_items = HashSet::new();
                if let Some(recents) = recents {
                    for entry in recents.sequence_values::<String>() {
                        recent_items.insert(entry?);
                    }
                }

                let mut out = Vec::new();
                let mut seen = HashSet::new();
                if needle.is_empty() {
                    for value in items.sequence_values::<LuaTable>() {
                        let item = value?;
                        let text = item.get::<String>(field.as_str())?;
                        if recent_items.contains(&text) && seen.insert(text.clone()) {
                            out.push(item);
                            if let Some(limit) = limit {
                                if out.len() >= limit {
                                    let table = lua.create_table_with_capacity(out.len(), 0)?;
                                    for (idx, item) in out.into_iter().enumerate() {
                                        table.raw_set((idx + 1) as i64, item)?;
                                    }
                                    return Ok(table);
                                }
                            }
                        }
                    }
                }

                let mut ranked = Vec::new();
                for value in items.sequence_values::<LuaTable>() {
                    let item = value?;
                    let text = item.get::<String>(field.as_str())?;
                    if let Some(score) = crate::editor::fuzzy_match(&text, &needle, files) {
                        ranked.push((item, text, score));
                    }
                }
                ranked.sort_by(|a, b| match b.2.cmp(&a.2) {
                    Ordering::Equal => a.1.cmp(&b.1),
                    other => other,
                });

                for (item, text, _) in ranked {
                    if seen.insert(text) {
                        out.push(item);
                        if let Some(limit) = limit {
                            if out.len() >= limit {
                                break;
                            }
                        }
                    }
                }

                let table = lua.create_table_with_capacity(out.len(), 0)?;
                for (idx, item) in out.into_iter().enumerate() {
                    table.raw_set((idx + 1) as i64, item)?;
                }
                Ok(table)
            },
        )?,
    )?;

    Ok(module)
}

#[cfg(test)]
mod tests {
    use super::rank_strings_inner;

    #[test]
    fn keeps_recents_first_on_empty_query() {
        let ranked = rank_strings_inner(
            vec![
                "src/main.rs".into(),
                "README.md".into(),
                "Cargo.toml".into(),
            ],
            "",
            true,
            &["README.md".into()],
            None,
        );
        assert_eq!(ranked.first().map(String::as_str), Some("README.md"));
    }

    #[test]
    fn ranks_matching_items_only() {
        let ranked = rank_strings_inner(
            vec!["alpha".into(), "beta".into(), "gamma".into()],
            "bt",
            false,
            &[],
            None,
        );
        assert_eq!(ranked, vec!["beta".to_string()]);
    }
}
