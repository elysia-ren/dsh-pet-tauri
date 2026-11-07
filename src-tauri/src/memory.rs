//! 对话记忆：`%APPDATA%/dsh-pet-tauri/memory.json`
//!
//! 结构：`{ "main": { "messages": [ { "role": "user"|"assistant", "content": "...", "ts": 123 } ] } }`
//! 全存不删；每次对话只取最近 N 轮进上下文。

use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn load() -> Value {
    let path = crate::settings::memory_path();
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .filter(|v| v.is_object())
        .unwrap_or_else(|| json!({}))
}

pub fn save(memory: &Value) {
    if let Ok(text) = serde_json::to_string_pretty(memory) {
        let _ = std::fs::write(crate::settings::memory_path(), text);
    }
}

pub fn messages(memory: &Value, bucket: &str) -> Vec<Value> {
    memory
        .get(bucket)
        .and_then(|b| b.get("messages"))
        .and_then(|m| m.as_array())
        .cloned()
        .unwrap_or_default()
}

pub fn append(memory: &mut Value, bucket: &str, role: &str, content: &str) {
    if !memory.is_object() {
        *memory = json!({});
    }
    let root = memory.as_object_mut().unwrap();
    let bucket_value = root
        .entry(bucket.to_string())
        .or_insert_with(|| json!({ "messages": [] }));
    if !bucket_value.is_object() {
        *bucket_value = json!({ "messages": [] });
    }
    let obj = bucket_value.as_object_mut().unwrap();
    let list = obj
        .entry("messages".to_string())
        .or_insert_with(|| json!([]));
    if !list.is_array() {
        *list = json!([]);
    }
    list.as_array_mut().unwrap().push(json!({
        "role": role,
        "content": content,
        "ts": now_ms(),
    }));
}

pub fn clear(memory: &mut Value, bucket: &str) {
    if let Some(obj) = memory.as_object_mut() {
        if let Some(b) = obj.get_mut(bucket).and_then(|v| v.as_object_mut()) {
            b.insert("messages".into(), json!([]));
        }
    }
}

/// 最近 N 轮（1 轮 = 1 问 1 答）；返回 (role, content) 列表（不含 system）。
pub fn recent(memory: &Value, bucket: &str, rounds: u32) -> Vec<(String, String)> {
    let list = messages(memory, bucket);
    let take = (rounds as usize) * 2;
    let start = list.len().saturating_sub(take);
    list[start..]
        .iter()
        .filter_map(|m| {
            let role = m.get("role")?.as_str()?.to_string();
            let content = m.get("content")?.as_str()?.to_string();
            Some((role, content))
        })
        .collect()
}

pub fn count(memory: &Value, bucket: &str) -> usize {
    messages(memory, bucket).len()
}
