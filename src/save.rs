//! `profile.toml` 저장.
//!
//! 편집 UI 가 보낸 설정을 **기존 파일에 덮어쓰지 않고 병합합니다.** 설정을
//! 그대로 직렬화해 쓰면 주석과 필드 순서가 전부 사라지는데, 이 파일은 손으로도
//! 고치는 파일이라 안내 주석이 곧 문서입니다.
//!
//! toml_edit 은 값만 바꾸면 그 줄의 공백·주석을 그대로 둡니다. 그래서 새 값
//! 트리를 돌면서 **있는 키는 값만 갈아끼우고**, 없어진 키만 지웁니다.
//!
//! 한계: 주석은 내용이 아니라 위치에 붙습니다. 편집 UI 에서 섹션 순서를 바꾸면
//! 주석은 제자리에 남아 엉뚱한 섹션을 설명하게 됩니다. 순서를 바꿀 때 경고를
//! 띄우는 것이 맞고, 지금은 문서에 적어두었습니다.

use std::fs;
use std::io;
use std::path::Path;

use serde_json::Value as Json;
use toml_edit::{Array, ArrayOfTables, DocumentMut, Formatted, Item, Table, Value};

/// 새 설정을 기존 문서에 병합해 저장합니다.
///
/// 원자적으로 씁니다 — 임시 파일에 먼저 쓰고 이름을 바꿉니다. 저장 도중
/// 프로그램이 멈춰도 반쯤 쓰인 설정이 남지 않습니다.
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
// 병합
// ─────────────────────────────────────────────────────────────────────────────

fn sync_table(table: &mut Table, map: &serde_json::Map<String, Json>) {
    // 새 설정에 없는 키를 먼저 지웁니다.
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
            // 값이 없는 선택 필드는 키를 지웁니다. 빈 문자열로 쓰면 "경로가
            // 지정됐는데 파일이 없다"는 오류로 둔갑합니다.
            Json::Null => {
                table.remove(key);
            }

            // 배열 안에 테이블만 있으면 [[key]] 꼴로 씁니다. 인라인 배열보다
            // 훨씬 읽기 쉽고, 손으로 쓴 파일의 모양과도 맞습니다.
            // 단, 이미 인라인 배열로 쓰여 있으면 그 모양을 존중합니다.
            Json::Array(items)
                if is_array_of_tables(items)
                    && !matches!(table.get(key), Some(Item::Value(Value::Array(_)))) =>
            {
                sync_array_of_tables(table, key, items);
            }

            // 일반 배열은 원소 단위로 갱신합니다. 통째로 갈아끼우면 여러 줄로
            // 써둔 배열이 한 줄로 뭉개집니다.
            Json::Array(items) => {
                if let Some(Item::Value(Value::Array(existing))) = table.get_mut(key) {
                    sync_array(existing, items);
                } else {
                    table[key] = Item::Value(to_value(value));
                }
            }

            Json::Object(inner) => {
                // 이미 인라인 테이블이면 그 모양과 키 순서를 유지합니다.
                // { ko = "...", en = "..." } 를 통째로 갈아끼우면 번역이 저장할
                // 때마다 알파벳순으로 재배열되어 diff 가 지저분해집니다.
                if let Some(Item::Value(Value::InlineTable(existing))) = table.get_mut(key) {
                    sync_inline_table(existing, inner);
                    continue;
                }
                let entry = table
                    .entry(key)
                    .or_insert_with(|| Item::Table(Table::new()));
                match entry.as_table_mut() {
                    Some(sub) => sync_table(sub, inner),
                    // 스칼라였던 자리가 테이블이 된 경우.
                    None => *entry = Item::Table(new_table(inner)),
                }
            }

            _ => {
                table[key] = Item::Value(to_value(value));
            }
        }
    }
}

/// 배열을 제자리에서 갱신합니다. 줄바꿈과 들여쓰기가 살아남습니다.
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

/// 값을 바꾸되 주변 공백은 그대로 둡니다.
///
/// toml_edit 에서 공백·줄바꿈은 값에 붙어 있습니다(decor). 그냥 대입하면
/// 여러 줄로 정렬해 둔 배열의 줄바꿈이 함께 사라집니다.
fn replace_value(slot: &mut Value, mut next: Value) {
    let decor = slot.decor().clone();
    *next.decor_mut() = decor;
    *slot = next;
}

/// 인라인 테이블을 제자리에서 갱신합니다. 기존 키는 순서를 지키고, 새 키만
/// 뒤에 붙으며, 사라진 키는 지웁니다.
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
            // 중첩 인라인 테이블도 같은 규칙으로.
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

/// 인라인 테이블 끝에 키를 붙입니다.
///
/// 직전 값에 붙어 있던 닫는 중괄호 앞 공백을 새 값 쪽으로 옮깁니다. 그대로 두면
/// `{ ko = "박" , ja = "パク" }` 처럼 쉼표 앞에 공백이 남습니다.
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
    // 기존이 [[key]] 가 아니면 새로 만듭니다.
    if table.get(key).and_then(Item::as_array_of_tables).is_none() {
        table[key] = Item::ArrayOfTables(ArrayOfTables::new());
    }
    let Some(array) = table[key].as_array_of_tables_mut() else {
        return;
    };

    // 길이를 맞춥니다. 줄어든 쪽은 뒤에서부터 지웁니다.
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

/// 테이블만 들어 있는 비어 있지 않은 배열인지.
fn is_array_of_tables(items: &[Json]) -> bool {
    !items.is_empty() && items.iter().all(|i| matches!(i, Json::Object(_)))
}

fn to_value(json: &Json) -> Value {
    match json {
        // 호출하는 쪽에서 null 은 미리 걸러냅니다. 배열 안에 섞여 들어온
        // 경우에만 여기까지 오고, 그때는 빈 문자열이 가장 덜 해롭습니다.
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

    /// 이 파일이 존재하는 이유. 주석이 살아남아야 합니다.
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

    /// 번역 표는 인라인 테이블 모양과 **키 순서**를 유지해야 합니다.
    /// 저장할 때마다 순서가 바뀌면 diff 가 실제 변경을 묻어버립니다.
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

    /// 새 언어는 뒤에 붙습니다.
    #[test]
    fn new_translation_key_is_appended() {
        let source = "[profile]\nname = { ko = \"박\" }\n";
        let out = merge(source, json!({"profile": {"name": {"ko": "박", "ja": "パク"}}}));
        assert!(out.contains("ko = \"박\", ja = \"パク\""), "{out}");
    }

    /// 값이 없는 선택 필드는 키가 사라져야 합니다. 빈 문자열로 남기면
    /// "경로가 지정됐는데 파일이 없다"는 오류로 둔갑합니다.
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

    /// 여러 줄로 써둔 배열이 한 줄로 뭉개지면 안 됩니다.
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
        // 정수가 실수로 바뀌면 TOML 타입이 달라져 파싱이 깨집니다.
        assert!(!out.contains("heading_weight = 800.0"));
    }

    /// 병합 결과가 다시 읽히는지 — 저장이 파일을 망가뜨리면 안 됩니다.
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
