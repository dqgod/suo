use std::time::Duration;

use reqwest::{
    blocking::Client,
    header::{HeaderMap, HeaderValue, CONTENT_TYPE},
};
use serde::{Deserialize, Serialize};

use crate::config::{read_translation_api_key, TranslationConfig};

const TRANSLATE_ENDPOINT: &str = "https://api.cognitive.microsofttranslator.com/translate";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct TranslationRequest<'a> {
    text: &'a str,
}

#[derive(Deserialize)]
struct TranslationResponse {
    translations: Vec<TranslationText>,
}

#[derive(Deserialize)]
struct TranslationText {
    text: String,
    to: String,
}

pub fn target_language(config: &TranslationConfig, text: &str) -> String {
    if contains_chinese(text) {
        config.chinese_target_language.clone()
    } else {
        config.default_target_language.clone()
    }
}

pub fn translate<F>(
    config: &TranslationConfig,
    text: &str,
    target: &str,
    is_cancelled: F,
) -> Result<String, String>
where
    F: Fn() -> bool,
{
    if text.chars().count() > 10_000 {
        return Err("单次翻译内容不能超过 10000 个字符".into());
    }
    let api_key = read_translation_api_key()?
        .ok_or_else(|| "尚未配置微软翻译 API 密钥，请在设置中添加".to_string())?;
    if is_cancelled() {
        return Err("翻译已取消".into());
    }

    let mut headers = HeaderMap::new();
    headers.insert(
        "Ocp-Apim-Subscription-Key",
        HeaderValue::from_str(&api_key).map_err(|_| "翻译 API 密钥格式无效".to_string())?,
    );
    if !config.region.is_empty() {
        headers.insert(
            "Ocp-Apim-Subscription-Region",
            HeaderValue::from_str(&config.region).map_err(|_| "翻译区域格式无效".to_string())?,
        );
    }
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    let client = Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .default_headers(headers)
        .build()
        .map_err(|error| format!("无法初始化翻译服务：{error}"))?;
    let response = client
        .post(TRANSLATE_ENDPOINT)
        .query(&[("api-version", "3.0"), ("to", target)])
        .json(&[TranslationRequest { text }])
        .send()
        .map_err(|error| format!("微软翻译请求失败：{error}"))?;
    if !response.status().is_success() {
        return Err(format!("微软翻译返回 HTTP {}", response.status()));
    }
    let response = response
        .json::<Vec<TranslationResponse>>()
        .map_err(|error| format!("无法解析翻译结果：{error}"))?;
    if is_cancelled() {
        return Err("翻译已取消".into());
    }
    let translation = response
        .into_iter()
        .next()
        .and_then(|item| item.translations.into_iter().next())
        .ok_or_else(|| "微软翻译没有返回结果".to_string())?;
    let _detected_target = translation.to;
    Ok(translation.text)
}

fn contains_chinese(text: &str) -> bool {
    text.chars().any(|character| {
        matches!(
            character as u32,
            0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_direction_from_input_text() {
        let config = TranslationConfig {
            enabled: true,
            keyword: "fy".into(),
            aliases: vec![],
            region: String::new(),
            default_target_language: "zh-Hans".into(),
            chinese_target_language: "en".into(),
        };
        assert_eq!(target_language(&config, "hello"), "zh-Hans");
        assert_eq!(target_language(&config, "你好"), "en");
    }
}
