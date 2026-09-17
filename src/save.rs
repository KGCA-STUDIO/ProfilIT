//! Saving `profile.toml`.
//!
//! The config sent by the editor UI is **merged into the existing file rather
//! than overwriting it.** Serializing the config straight to disk would wipe
//! out all comments and field ordering, and since this file is also meant to
//! be hand-edited, its guiding comments effectively are the documentation.
//!
//! toml_edit leaves a line's whitespace and comments alone as long as only the
//! value changes. So we walk the new value tree and **swap values in place for
//! keys that already exist**, only removing keys that are actually gone.
//!
//! Known limitation: comments are attached by position, not by content. If the
//! editor UI reorders sections, a comment stays put and ends up describing the
//! wrong section. The right fix is warning on reorder; for now it's just
//! written down here.

use std::fs;
use std::io;
use std::path::Path;

use serde_json::Value as Json;
use toml_edit::{Array, ArrayOfTables, DocumentMut, Formatted, Item, Table, Value};

/// Merges the new config into the existing document and saves it.
///
/// The write is atomic — we write to a temp file first, then rename it. That
/// way, even if the program dies mid-save, we never end up with a half-written config.
pub fn save(path: &Path, config: &Json) -> io::Result<String> {
    let existing = fs::read_to_string(path).unwrap_or_default();
    let mut doc: DocumentMut = existing
        .parse()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("{e}")))?;

    let Json::Object(map) = config else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "설정은 최상위가 테이블이어야 합니다.",
        ));
    };

    sync_table(doc.as_table_mut(), map);
    let rendered = doc.to_string();

    let temp = path.with_extension("toml.tmp");
    fs::write(&temp, &rendered)?;
    fs::rename(&temp, path)?;

    Ok(rendered)
}

// ─────────────────────────────────────────────────────────────────────────────
// Merging
// ─────────────────────────────────────────────────────────────────────────────

fn sync_table(table: &mut Table, map: &serde_json::Map<String, Json>) {
    // First, remove any keys that aren't in the new config.
    let stale: Vec<String> = table
        .iter()
        .map(|(k, _)| k.to_string())
        .filter(|k| !map.contains_key(k))
        .collect();
    for key in stale {
        table.remove(&key);
    }

    for (key, value) in map {
        match value {
            // An optional field with no value gets its key removed. Writing an
            // empty string instead would turn into a "path is set but the file
            // doesn't exist" error.
            Json::Null => {
                table.remove(key);
            }

            // An array containing only tables is written as `[[key]]`. It's
            // far more readable than an inline array and matches how these
            // files look when hand-written. If it's already an inline array
            // though, we respect that existing shape.
            Json::Array(items)
                if is_array_of_tables(items)
                    && !matches!(table.get(key), Some(Item::Value(Value::Array(_)))) =>
            {
                sync_array_of_tables(table, key, items);
            }

            // A plain array is updated element by element. Replacing it wholesale
            // would collapse an array written across multiple lines into one line.
            Json::Array(items) => {
                if let Some(Item::Value(Value::Array(existing))) = table.get_mut(key) {
                    sync_array(existing, items);
                } else {
                    table[key] = Item::Value(to_value(value));
                }
            }

            Json::Object(inner) => {
                // If it's already an inline table, we keep its shape and key
                // order. Replacing `{ ko = "...", en = "..." }` wholesale would
                // re-sort translations alphabetically on every save, making
                // diffs a mess.
                if let Some(Item::Value(Value::InlineTable(existing))) = table.get_mut(key) {
                    sync_inline_table(existing, inner);
                    continue;
                }
                let entry = table
                    .entry(key)
                    .or_insert_with(|| Item::Table(Table::new()));
                match entry.as_table_mut() {
                    Some(sub) => sync_table(sub, inner),
                    // The slot used to hold a scalar and is now a table.
                    None => *entry = Item::Table(new_table(inner)),
                }
            }

            _ => {
                table[key] = Item::Value(to_value(value));
            }
        }
    }
}

/// Updates an array in place, preserving line breaks and indentation.
fn sync_array(array: &mut Array, items: &[Json]) {
    while array.len() > items.len() {
        array.remove(array.len() - 1);
    }

    for (i, item) in items.iter().enumerate() {
        if i >= array.len() {
            array.push(to_value(item));
            continue;
        }
        match (array.get_mut(i), item) {
            (Some(Value::InlineTable(existing)), Json::Object(map)) => {
                sync_inline_table(existing, map);
            }
            (Some(slot), _) => replace_value(slot, to_value(item)),
            (None, _) => {}
        }
    }
}

/// Replaces the value while leaving its surrounding whitespace alone.
///
/// In toml_edit, whitespace and line breaks are attached to the value itself
/// (as "decor"). A plain assignment would wipe out the line breaks of an
/// array formatted across multiple lines.
fn replace_value(slot: &mut Value, mut next: Value) {
    let decor = slot.decor().clone();
    *next.decor_mut() = decor;
    *slot = next;
}

/// Updates an inline table in place. Existing keys keep their order, new keys
/// are appended, and removed keys are deleted.
fn sync_inline_table(table: &mut toml_edit::InlineTable, map: &serde_json::Map<String, Json>) {
    let stale: Vec<String> = table
        .iter()
        .map(|(k, _)| k.to_string())
        .filter(|k| !map.contains_key(k) || map[k].is_null())
        .collect();
    for key in stale {
        table.remove(&key);
    }

    for (key, value) in map {
        if value.is_null() {
            continue;
        }
        match table.get_mut(key) {
            // A nested inline table follows the same rules.
            Some(Value::InlineTable(inner)) => {
                if let Json::Object(sub) = value {
                    sync_inline_table(inner, sub);
                    continue;
                }
                replace_value(table.get_mut(key).unwrap(), to_value(value));
            }
            Some(slot) => replace_value(slot, to_value(value)),
            None => append_to_inline(table, key, to_value(value)),
        }
    }
}

/// Appends a key to the end of an inline table.
///
/// Moves the whitespace that sat before the closing brace on the previous
/// value over to the new value. Without this, you'd end up with a stray space
/// before the comma, like `{ ko = "박" , ja = "パク" }`.
fn append_to_inline(table: &mut toml_edit::InlineTable, key: &str, mut value: Value) {
    let last = table.iter().last().map(|(k, _)| k.to_string());
    let mut suffix = " ".to_string();

    if let Some(last) = last {
        if let Some(previous) = table.get_mut(&last) {
            if let Some(existing) = previous.decor().suffix().and_then(|s| s.as_str()) {
                suffix = existing.to_string();
            }
            previous.decor_mut().set_suffix("");
        }
    }

    value.decor_mut().set_prefix(" ");
    value.decor_mut().set_suffix(suffix);
    table.insert(key, value);
}

fn sync_array_of_tables(table: &mut Table, key: &str, items: &[Json]) {
    // If the existing value isn't `[[key]]`, create it fresh.
    if table.get(key).and_then(Item::as_array_of_tables).is_none() {
        table[key] = Item::ArrayOfTables(ArrayOfTables::new());
    }
    let Some(array) = table[key].as_array_of_tables_mut() else {
        return;
    };

    // Match the lengths up; if it shrank, trim from the end.
    while array.len() > items.len() {
        array.remove(array.len() - 1);
    }
    while array.len() < items.len() {
        array.push(Table::new());
    }

    for (i, item) in items.iter().enumerate() {
        let Json::Object(map) = item else { continue };
        if let Some(target) = array.get_mut(i) {
            sync_table(target, map);
        }
    }
}

fn new_table(map: &serde_json::Map<String, Json>) -> Table {
    let mut table = Table::new();
    sync_table(&mut table, map);
    table
}

/// Whether this is a non-empty array containing only tables.
fn is_array_of_tables(items: &[Json]) -> bool {
    !items.is_empty() && items.iter().all(|i| matches!(i, Json::Object(_)))
}

fn to_value(json: &Json) -> Value {
    match json {
        // Callers already filter out null before this point. It only reaches
        // here when null shows up mixed into an array, in which case an empty
        // string is the least harmful thing to write.
        Json::Null => Value::String(Formatted::new(String::new())),
        Json::Bool(b) => Value::Boolean(Formatted::new(*b)),
        Json::Number(n) => match (n.as_i64(), n.as_f64()) {
            (Some(i), _) => Value::Integer(Formatted::new(i)),
            (None, Some(f)) => Value::Float(Formatted::new(f)),
            _ => Value::String(Formatted::new(n.to_string())),
        },
        Json::String(s) => Value::String(Formatted::new(s.clone())),
        Json::Array(items) => {
            let mut array = Array::new();
            for item in items {
                array.push(to_value(item));
            }
            Value::Array(array)
        }
        Json::Object(map) => {
            let mut inline = toml_edit::InlineTable::new();
            for (k, v) in map {
                inline.insert(k, to_value(v));
            }
            Value::InlineTable(inline)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn merge(source: &str, config: Json) -> String {
        let mut doc: DocumentMut = source.parse().unwrap();
        let Json::Object(map) = &config else { panic!() };
        sync_table(doc.as_table_mut(), map);
        doc.to_string()
    }

    /// The whole reason this file exists: comments have to survive.
    #[test]
    fn comments_survive_a_value_change() {
        let source = "\
# 이 주석은 살아남아야 합니다
[site]
# 사이트 제목
title = \"옛 제목\"
lang = \"ko\"
";
        let out = merge(source, json!({"site": {"title": "새 제목", "lang": "ko"}}));

        assert!(out.contains("# 이 주석은 살아남아야 합니다"));
        assert!(out.contains("# 사이트 제목"));
        assert!(out.contains("title = \"새 제목\""));
        assert!(!out.contains("옛 제목"));
    }

    #[test]
    fn removed_keys_disappear() {
        let source = "[site]\ntitle = \"제목\"\nstale = \"지워질 값\"\n";
        let out = merge(source, json!({"site": {"title": "제목"}}));
        assert!(!out.contains("stale"));
    }

    /// A translation table must keep its inline-table shape and **key order**.
    /// If the order shifted on every save, the diff would bury the real change.
    #[test]
    fn translation_tables_stay_inline_and_keep_key_order() {
        let source = "[profile]\nname = { ko = \"박\", en = \"Park\" }\n";
        let out = merge(
            source,
            json!({"profile": {"name": {"en": "Kim", "ko": "김"}}}),
        );
        assert!(out.contains("name = {"), "인라인 테이블이 깨졌습니다: {out}");
        assert!(!out.contains("[profile.name]"));
        assert!(
            out.contains("ko = \"김\", en = \"Kim\""),
            "키 순서가 바뀌었습니다: {out}"
        );
    }

    /// A new language gets appended at the end.
    #[test]
    fn new_translation_key_is_appended() {
        let source = "[profile]\nname = { ko = \"박\" }\n";
        let out = merge(source, json!({"profile": {"name": {"ko": "박", "ja": "パク"}}}));
        assert!(out.contains("ko = \"박\", ja = \"パク\""), "{out}");
    }

    /// An optional field with no value should have its key removed entirely.
    /// Leaving an empty string behind would turn into a "path is set but the
    /// file doesn't exist" error.
    #[test]
    fn null_removes_the_key_instead_of_writing_empty_string() {
        let source = "[site]\ntitle = \"제목\"\n";
        let out = merge(
            source,
            json!({"site": {"title": "제목", "og_image": null, "favicon": null}}),
        );
        assert!(!out.contains("og_image"), "{out}");
        assert!(!out.contains("favicon"), "{out}");
        assert!(!out.contains("\"\""), "빈 문자열이 남았습니다: {out}");
    }

    /// An array written across multiple lines must not collapse into one line.
    #[test]
    fn multiline_arrays_keep_their_shape() {
        let source = "\
[[sections]]
type = \"tags\"
items = [
  \"Rust\",
  { ko = \"요리\", en = \"Cooking\" },
]
";
        let out = merge(
            source,
            json!({"sections": [{
                "type": "tags",
                "items": ["Rust", {"en": "Cooking", "ko": "쿠킹"}],
            }]}),
        );
        assert!(out.contains("items = [\n"), "배열이 한 줄로 뭉개졌습니다: {out}");
        assert!(out.contains("ko = \"쿠킹\", en = \"Cooking\""), "{out}");
    }

    #[test]
    fn null_clears_an_existing_key() {
        let source = "[profile]\nname = \"박\"\navatar = \"assets/a.jpg\"\n";
        let out = merge(source, json!({"profile": {"name": "박", "avatar": null}}));
        assert!(!out.contains("avatar"), "{out}");
    }

    #[test]
    fn array_of_tables_keeps_per_item_comments() {
        let source = "\
[[sections]]
# 첫 섹션 주석
type = \"tags\"
title = \"관심사\"

[[sections]]
type = \"about\"
body = \"소개\"
";
        let out = merge(
            source,
            json!({"sections": [
                {"type": "tags", "title": "취미"},
                {"type": "about", "body": "소개"},
            ]}),
        );
        assert!(out.contains("# 첫 섹션 주석"));
        assert!(out.contains("title = \"취미\""));
    }

    #[test]
    fn array_of_tables_shrinks_and_grows() {
        let source = "[[sections]]\ntype = \"tags\"\n\n[[sections]]\ntype = \"about\"\n";

        let shrunk = merge(source, json!({"sections": [{"type": "tags"}]}));
        assert_eq!(shrunk.matches("[[sections]]").count(), 1);

        let grown = merge(
            source,
            json!({"sections": [{"type": "tags"}, {"type": "about"}, {"type": "links"}]}),
        );
        assert_eq!(grown.matches("[[sections]]").count(), 3);
    }

    #[test]
    fn numbers_keep_their_kind() {
        let source = "[theme.font]\nheading_weight = 700\nline_height = 1.6\n";
        let out = merge(
            source,
            json!({"theme": {"font": {"heading_weight": 800, "line_height": 1.75}}}),
        );
        assert!(out.contains("heading_weight = 800"));
        assert!(out.contains("line_height = 1.75"));
        // If an integer turned into a float, the TOML type would change and break parsing.
        assert!(!out.contains("heading_weight = 800.0"));
    }

    /// Whether the merged output can be parsed again — saving must never corrupt the file.
    #[test]
    fn merged_output_parses_again() {
        let source = "[site]\ntitle = \"제목\"\n";
        let out = merge(
            source,
            json!({
                "schema_version": 1,
                "site": {"title": "제목", "languages": ["ko", "en"]},
                "sections": [{"type": "tags", "items": ["가", "나"]}],
            }),
        );
        let reparsed: toml::Table = out.parse().expect("병합 결과를 다시 읽을 수 없습니다");
        assert_eq!(reparsed["site"]["languages"][1].as_str(), Some("en"));
    }
}
