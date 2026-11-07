//! DeepSeek 官方余额接口（`GET {base}/user/balance`），以及档位动画映射。
//!
//! 档位口径沿用 dsh-pet：
//!   percent = 已用百分比（余额按 `balance_full` 基准折算；DeepSeek 默认 ¥20）
//!   index = percent >= 100 ? 5 : floor(percent / 20)，上限 4
//! 动画顺序 = 满 → 空：钱袋满溢 / 金袋叮当 / 钱袋如常 / 数金皱眉 / 袋空如洗 / 分文不剩。

use serde::Serialize;
use std::time::Duration;

pub const BALANCE_ANIMS: [&str; 6] = [
    "余额-钱袋满溢",
    "余额-金袋叮当",
    "余额-钱袋如常",
    "余额-数金皱眉",
    "余额-袋空如洗",
    "余额-分文不剩",
];

#[derive(Serialize)]
pub struct BalanceOut {
    pub ok: bool,
    pub currency: String,
    pub total: f64,
    pub granted: f64,
    pub topped_up: f64,
    pub tier_index: usize,
    pub text: String,
    pub anim: String,
}

fn balance_endpoint(base_url: &str) -> String {
    let mut root = base_url.trim_end_matches('/').to_string();
    if let Some(stripped) = root.strip_suffix("/v1") {
        root = stripped.to_string();
    }
    format!("{root}/user/balance")
}

pub fn tier_index(total: f64, full: f64) -> usize {
    if full <= 0.0 {
        return 0;
    }
    let used = (100.0 - (total.max(0.0) / full) * 100.0).clamp(0.0, 100.0);
    if used >= 100.0 {
        5
    } else {
        ((used / 20.0).floor() as usize).min(4)
    }
}

pub async fn fetch(base_url: &str, key: &str, full: f64) -> Result<BalanceOut, String> {
    if key.trim().is_empty() {
        return Err("未设置 API Key（右键 → 设置 → AI 模型）".into());
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| format!("HTTP 客户端初始化失败：{e}"))?;

    let url = balance_endpoint(base_url);
    let resp = client
        .get(&url)
        .bearer_auth(key)
        .send()
        .await
        .map_err(|e| format!("余额请求失败：{e}"))?;

    let status = resp.status();
    let text = resp.text().await.map_err(|e| format!("读取余额响应失败：{e}"))?;
    if !status.is_success() {
        let brief: String = text.chars().take(300).collect();
        return Err(format!("余额接口返回 {status}：{brief}"));
    }

    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("余额响应不是合法 JSON：{e}"))?;
    let info = value
        .get("balance_infos")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .ok_or_else(|| "余额响应缺少 balance_infos".to_string())?;

    let num = |key: &str| -> f64 {
        info.get(key)
            .and_then(|v| {
                v.as_f64()
                    .or_else(|| v.as_str().and_then(|s| s.parse::<f64>().ok()))
            })
            .unwrap_or(0.0)
    };

    let currency = info
        .get("currency")
        .and_then(|v| v.as_str())
        .unwrap_or("CNY")
        .to_string();
    let total = num("total_balance");
    let granted = num("granted_balance");
    let topped_up = num("topped_up_balance");
    let index = tier_index(total, full);
    let text = if currency == "CNY" {
        format!("余额 ¥{total:.2}")
    } else {
        format!("余额 {currency} {total:.2}")
    };

    Ok(BalanceOut {
        ok: true,
        currency,
        total,
        granted,
        topped_up,
        tier_index: index,
        text,
        anim: BALANCE_ANIMS[index].to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::tier_index;

    #[test]
    fn tiers_follow_dsh_rules() {
        assert_eq!(tier_index(20.0, 20.0), 0); // 满
        assert_eq!(tier_index(15.0, 20.0), 1); // 25% 已用
        assert_eq!(tier_index(10.0, 20.0), 2); // 50%
        assert_eq!(tier_index(5.0, 20.0), 3); // 75%
        assert_eq!(tier_index(0.0, 20.0), 5); // 用尽
        assert_eq!(tier_index(-1.0, 20.0), 5); // 透支
    }
}
