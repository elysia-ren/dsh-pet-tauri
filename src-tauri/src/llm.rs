//! OpenAI 兼容的对话补全客户端（DeepSeek / OpenAI / 本地 Ollama 等）。
//!
//! 独立桌宠不再依赖 DSH 的 `llm` 服务：直接 `POST {base_url}/chat/completions`。
//! 只做非流式（一次拿全文），对桌宠的碎碎念/对话足够，且实现简单。

use serde::Serialize;
use std::time::Duration;

#[derive(Clone, Serialize)]
pub struct Msg {
    pub role: String,
    pub content: String,
}

impl Msg {
    pub fn new(role: &str, content: &str) -> Self {
        Self {
            role: role.into(),
            content: content.into(),
        }
    }
}

#[derive(Serialize)]
struct Request<'a> {
    model: &'a str,
    messages: &'a [Msg],
    temperature: f64,
    stream: bool,
    max_tokens: u32,
}

fn endpoint(base_url: &str) -> String {
    format!(
        "{}/chat/completions",
        base_url.trim_end_matches('/')
    )
}

/// 生成一段文本。返回模型正文；失败返回可展示的中文原因。
pub async fn complete(
    base_url: &str,
    key: &str,
    model: &str,
    messages: &[Msg],
    temperature: f64,
) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| format!("HTTP 客户端初始化失败：{e}"))?;

    let body = Request {
        model,
        messages,
        temperature,
        stream: false,
        max_tokens: 512,
    };

    // 本地 Ollama / 免鉴权端点允许空 Key
    let mut request = client.post(endpoint(base_url)).json(&body);
    if !key.trim().is_empty() {
        request = request.bearer_auth(key);
    }
    let resp = request
        .send()
        .await
        .map_err(|e| format!("请求失败：{e}"))?;

    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|e| format!("读取响应失败：{e}"))?;

    if !status.is_success() {
        let brief: String = text.chars().take(400).collect();
        return Err(format!("模型接口返回 {status}：{brief}"));
    }

    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("响应不是合法 JSON：{e}"))?;

    if let Some(err) = value.get("error") {
        let message = err
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("未知错误");
        return Err(format!("模型接口错误：{message}"));
    }

    let content = value
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    if content.is_empty() {
        return Err("模型没有返回正文".into());
    }
    Ok(content)
}
