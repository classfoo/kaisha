use axum::http::HeaderMap;
use std::{collections::HashMap, sync::OnceLock};

type Dict = HashMap<String, String>;

fn parse_lang(headers: &HeaderMap) -> &'static str {
    if let Some(lang) = headers.get("x-lang").and_then(|v| v.to_str().ok()) {
        if lang.starts_with("zh") {
            return "zh";
        }
        if lang.starts_with("ja") {
            return "ja";
        }
    }

    if let Some(lang) = headers.get("accept-language").and_then(|v| v.to_str().ok()) {
        if lang.starts_with("zh") {
            return "zh";
        }
        if lang.starts_with("ja") {
            return "ja";
        }
    }
    "en"
}

fn load_dict(raw: &str) -> Dict {
    serde_json::from_str::<Dict>(raw).unwrap_or_default()
}

fn dict(lang: &str) -> &'static Dict {
    static EN: OnceLock<Dict> = OnceLock::new();
    static ZH: OnceLock<Dict> = OnceLock::new();
    static JA: OnceLock<Dict> = OnceLock::new();
    match lang {
        "zh" => ZH.get_or_init(|| load_dict(include_str!("../i18n/zh.json"))),
        "ja" => JA.get_or_init(|| load_dict(include_str!("../i18n/ja.json"))),
        _ => EN.get_or_init(|| load_dict(include_str!("../i18n/en.json"))),
    }
}

pub fn msg(headers: &HeaderMap, key: &str) -> String {
    let lang = parse_lang(headers);
    if let Some(v) = dict(lang).get(key) {
        return v.clone();
    }
    if let Some(v) = dict("en").get(key) {
        return v.clone();
    }
    key.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, HeaderValue};

    fn headers_with(lang: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("x-lang", HeaderValue::from_str(lang).unwrap());
        headers
    }

    #[test]
    fn msg_returns_localized_string_for_zh() {
        let headers = headers_with("zh");
        let text = msg(&headers, "employee_not_found");
        assert!(text.contains("员工") || text.contains("找不到"));
    }

    #[test]
    fn msg_falls_back_to_en_for_unknown_lang() {
        let headers = headers_with("fr");
        let text = msg(&headers, "employee_not_found");
        assert!(text.contains("Employee") || text.contains("employee"));
    }

    #[test]
    fn msg_returns_key_when_missing() {
        let headers = HeaderMap::new();
        assert_eq!(msg(&headers, "nonexistent_key_xyz"), "nonexistent_key_xyz");
    }

    #[test]
    fn msg_resolves_task_not_found_in_all_locales() {
        for lang in ["en", "zh", "ja"] {
            let mut headers = HeaderMap::new();
            headers.insert("x-lang", HeaderValue::from_str(lang).unwrap());
            let text = msg(&headers, "task_not_found");
            assert_ne!(text, "task_not_found");
        }
    }
}
