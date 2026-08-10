use std::time::{Duration, SystemTime, UNIX_EPOCH};

use reqwest::{
    blocking::Client,
    header::{HeaderMap, HeaderValue, CONTENT_TYPE},
    StatusCode,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::config::{
    read_translation_credentials, TranslationConfig, TranslationCredentials, TranslationProvider,
};

const MICROSOFT_TRANSLATE_ENDPOINT: &str =
    "https://api.cognitive.microsofttranslator.com/translate";
const GOOGLE_TRANSLATE_ENDPOINT: &str = "https://translation.googleapis.com/language/translate/v2";
const YOUDAO_TRANSLATE_ENDPOINT: &str = "https://openapi.youdao.com/api";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct MicrosoftRequest<'a> {
    text: &'a str,
}

#[derive(Deserialize)]
struct MicrosoftResponse {
    translations: Vec<MicrosoftTranslation>,
}

#[derive(Deserialize)]
struct MicrosoftTranslation {
    text: String,
    to: String,
}

#[derive(Serialize)]
struct GoogleRequest<'a> {
    q: &'a str,
    target: &'a str,
    format: &'static str,
}

#[derive(Deserialize)]
struct GoogleResponse {
    data: GoogleResponseData,
}

#[derive(Deserialize)]
struct GoogleResponseData {
    translations: Vec<GoogleTranslation>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoogleTranslation {
    translated_text: String,
}

#[derive(Serialize)]
struct YoudaoRequest<'a> {
    q: &'a str,
    #[serde(rename = "from")]
    source_language: &'static str,
    #[serde(rename = "to")]
    target_language: &'a str,
    #[serde(rename = "appKey")]
    app_key: &'a str,
    salt: &'a str,
    sign: &'a str,
    #[serde(rename = "signType")]
    sign_type: &'static str,
    curtime: &'a str,
}

#[derive(Deserialize)]
struct YoudaoResponse {
    #[serde(rename = "errorCode")]
    error_code: Value,
    #[serde(default)]
    translation: Vec<String>,
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
    let maximum = match config.provider {
        TranslationProvider::Youdao => 5_000,
        TranslationProvider::Microsoft | TranslationProvider::Google => 10_000,
    };
    if text.chars().count() > maximum {
        return Err(format!(
            "{} 单次翻译内容不能超过 {maximum} 个字符",
            config.provider.display_name()
        ));
    }
    let credentials = read_translation_credentials(config.provider)?.ok_or_else(|| {
        format!(
            "尚未配置 {} 凭据，请在设置中添加",
            config.provider.display_name()
        )
    })?;
    if is_cancelled() {
        return Err("翻译已取消".into());
    }

    let provider_target = provider_language(config.provider, target);
    let result = match (config.provider, credentials) {
        (TranslationProvider::Microsoft, TranslationCredentials::ApiKey(api_key)) => {
            translate_microsoft(config, text, &provider_target, &api_key)
        }
        (TranslationProvider::Google, TranslationCredentials::ApiKey(api_key)) => {
            translate_google(text, &provider_target, &api_key)
        }
        (
            TranslationProvider::Youdao,
            TranslationCredentials::Youdao {
                app_key,
                app_secret,
            },
        ) => translate_youdao(text, &provider_target, &app_key, &app_secret),
        _ => Err("翻译凭据与当前提供方不匹配，请在设置中重新保存".into()),
    }?;

    if is_cancelled() {
        return Err("翻译已取消".into());
    }
    Ok(result)
}

fn translate_microsoft(
    config: &TranslationConfig,
    text: &str,
    target: &str,
    api_key: &str,
) -> Result<String, String> {
    let mut headers = HeaderMap::new();
    headers.insert(
        "Ocp-Apim-Subscription-Key",
        HeaderValue::from_str(api_key).map_err(|_| "翻译 API 密钥格式无效".to_string())?,
    );
    if !config.region.is_empty() {
        headers.insert(
            "Ocp-Apim-Subscription-Region",
            HeaderValue::from_str(&config.region).map_err(|_| "翻译区域格式无效".to_string())?,
        );
    }
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    let response = Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .default_headers(headers)
        .build()
        .map_err(|error| format!("无法初始化 Microsoft Translator：{error}"))?
        .post(MICROSOFT_TRANSLATE_ENDPOINT)
        .query(&[("api-version", "3.0"), ("to", target)])
        .json(&[MicrosoftRequest { text }])
        .send()
        .map_err(|error| provider_request_error(TranslationProvider::Microsoft, &error))?;
    if !response.status().is_success() {
        return Err(provider_http_error(
            TranslationProvider::Microsoft,
            response.status(),
        ));
    }
    let response = response
        .json::<Vec<MicrosoftResponse>>()
        .map_err(|error| format!("无法解析 Microsoft Translator 结果：{error}"))?;
    let translation = response
        .into_iter()
        .next()
        .and_then(|item| item.translations.into_iter().next())
        .ok_or_else(|| "Microsoft Translator 没有返回结果".to_string())?;
    let _detected_target = translation.to;
    Ok(translation.text)
}

fn translate_google(text: &str, target: &str, api_key: &str) -> Result<String, String> {
    let response = Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|error| format!("无法初始化 Google 翻译：{error}"))?
        .post(GOOGLE_TRANSLATE_ENDPOINT)
        .query(&[("key", api_key)])
        .json(&GoogleRequest {
            q: text,
            target,
            format: "text",
        })
        .send()
        // The API key is a required system query parameter for Translation
        // Basic. Never format reqwest's error here because it may include the
        // complete request URL and therefore the key.
        .map_err(|error| provider_request_error(TranslationProvider::Google, &error))?;
    if !response.status().is_success() {
        return Err(provider_http_error(
            TranslationProvider::Google,
            response.status(),
        ));
    }
    let response = response
        .json::<GoogleResponse>()
        // Decoding errors can retain request metadata. Keep the message
        // provider-specific but never format an error that may own the URL.
        .map_err(|_| "无法解析 Google 翻译结果".to_string())?;
    response
        .data
        .translations
        .into_iter()
        .next()
        .map(|translation| translation.translated_text)
        .ok_or_else(|| "Google 翻译没有返回结果".to_string())
}

fn translate_youdao(
    text: &str,
    target: &str,
    app_key: &str,
    app_secret: &str,
) -> Result<String, String> {
    let curtime = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "系统时间早于 Unix 纪元，无法生成有道翻译签名".to_string())?
        .as_secs()
        .to_string();
    let salt = Uuid::new_v4().to_string();
    let sign = youdao_signature(app_key, text, &salt, &curtime, app_secret);
    let request = YoudaoRequest {
        q: text,
        source_language: "auto",
        target_language: target,
        app_key,
        salt: &salt,
        sign: &sign,
        sign_type: "v3",
        curtime: &curtime,
    };
    let response = Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|error| format!("无法初始化有道翻译：{error}"))?
        .post(YOUDAO_TRANSLATE_ENDPOINT)
        .form(&request)
        .send()
        .map_err(|error| provider_request_error(TranslationProvider::Youdao, &error))?;
    if !response.status().is_success() {
        return Err(provider_http_error(
            TranslationProvider::Youdao,
            response.status(),
        ));
    }
    let response = response
        .json::<YoudaoResponse>()
        .map_err(|error| format!("无法解析有道翻译结果：{error}"))?;
    parse_youdao_response(response)
}

fn provider_http_error(provider: TranslationProvider, status: StatusCode) -> String {
    let name = provider.display_name();
    match status.as_u16() {
        401 | 403 => format!("{name}鉴权失败，请检查当前提供方的凭据和服务权限"),
        429 => format!("{name}请求过于频繁或额度不足，请稍后重试"),
        400 => format!("{name}拒绝了请求，请检查目标语言和服务配置"),
        _ => format!("{name}返回 HTTP {status}"),
    }
}

fn provider_request_error(provider: TranslationProvider, error: &reqwest::Error) -> String {
    let name = provider.display_name();
    if error.is_timeout() {
        format!("{name}请求超时，请稍后重试")
    } else if error.is_connect() {
        format!("无法连接{name}，请检查网络连接")
    } else {
        format!("{name}请求失败，请检查网络连接或服务配置")
    }
}

fn parse_youdao_response(response: YoudaoResponse) -> Result<String, String> {
    let code = match response.error_code {
        Value::String(value) => value,
        Value::Number(value) => value.to_string(),
        _ => return Err("有道翻译返回了无法识别的错误码".into()),
    };
    if code != "0" {
        return Err(match code.as_str() {
            "101" | "113" => "有道翻译请求参数不完整".into(),
            "102" => "有道翻译不支持当前语言方向".into(),
            "103" => "有道翻译文本过长".into(),
            "108" | "111" | "202" | "205" | "206" => {
                "有道翻译鉴权失败，请检查应用 ID、应用密钥和服务类型".into()
            }
            "110" | "112" => "有道翻译应用尚未绑定有效的文本翻译服务".into(),
            "203" => "当前 IP 不在有道翻译应用的允许列表中".into(),
            "401" => "有道翻译账户余额不足".into(),
            "411" | "412" => "有道翻译请求过于频繁，请稍后重试".into(),
            _ => format!("有道翻译返回错误码 {code}"),
        });
    }
    response
        .translation
        .into_iter()
        .next()
        .ok_or_else(|| "有道翻译没有返回结果".to_string())
}

fn provider_language(provider: TranslationProvider, language: &str) -> String {
    let normalized = language.trim().to_ascii_lowercase();
    let simplified = matches!(normalized.as_str(), "zh" | "zh-cn" | "zh-hans" | "zh-chs");
    let traditional = matches!(normalized.as_str(), "zh-tw" | "zh-hant" | "zh-cht");
    match (provider, simplified, traditional) {
        (TranslationProvider::Microsoft, true, _) => "zh-Hans".into(),
        (TranslationProvider::Microsoft, _, true) => "zh-Hant".into(),
        (TranslationProvider::Google, true, _) => "zh-CN".into(),
        (TranslationProvider::Google, _, true) => "zh-TW".into(),
        (TranslationProvider::Youdao, true, _) => "zh-CHS".into(),
        (TranslationProvider::Youdao, _, true) => "zh-CHT".into(),
        _ => language.trim().to_string(),
    }
}

fn youdao_signature(
    app_key: &str,
    text: &str,
    salt: &str,
    curtime: &str,
    app_secret: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(app_key.as_bytes());
    hasher.update(youdao_signature_input(text).as_bytes());
    hasher.update(salt.as_bytes());
    hasher.update(curtime.as_bytes());
    hasher.update(app_secret.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn youdao_signature_input(text: &str) -> String {
    let characters = text.chars().collect::<Vec<_>>();
    if characters.len() <= 20 {
        return text.to_string();
    }
    let first = characters[..10].iter().collect::<String>();
    let last = characters[characters.len() - 10..]
        .iter()
        .collect::<String>();
    format!("{first}{}{last}", characters.len())
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

    fn translation_config(provider: TranslationProvider) -> TranslationConfig {
        TranslationConfig {
            enabled: true,
            keyword: "fy".into(),
            description: String::new(),
            aliases: vec![],
            provider,
            region: String::new(),
            default_target_language: "zh-Hans".into(),
            chinese_target_language: "en".into(),
        }
    }

    #[test]
    fn selects_direction_from_input_text() {
        let config = translation_config(TranslationProvider::Microsoft);
        assert_eq!(target_language(&config, "hello"), "zh-Hans");
        assert_eq!(target_language(&config, "你好"), "en");
    }

    #[test]
    fn maps_common_chinese_codes_for_each_provider() {
        assert_eq!(
            provider_language(TranslationProvider::Microsoft, "zh-CN"),
            "zh-Hans"
        );
        assert_eq!(
            provider_language(TranslationProvider::Google, "zh-Hans"),
            "zh-CN"
        );
        assert_eq!(
            provider_language(TranslationProvider::Youdao, "zh-Hans"),
            "zh-CHS"
        );
        assert_eq!(
            provider_language(TranslationProvider::Youdao, "zh-TW"),
            "zh-CHT"
        );
        assert_eq!(provider_language(TranslationProvider::Google, "ja"), "ja");
    }

    #[test]
    fn builds_youdao_v3_signature_input_by_characters() {
        assert_eq!(youdao_signature_input("short text"), "short text");
        assert_eq!(
            youdao_signature_input("abcdefghijklmnopqrstu"),
            "abcdefghij21lmnopqrstu"
        );
        assert_eq!(
            youdao_signature("test-app", "hello", "salt", "1700000000", "test-credential"),
            "f9f9ab02512c2e642018d6d08c353ac2a4daf90d69c41d478fa4ae16caf4b04b"
        );
    }

    #[test]
    fn parses_google_and_youdao_results_without_network_access() {
        let google: GoogleResponse =
            serde_json::from_str(r#"{"data":{"translations":[{"translatedText":"你好"}]}}"#)
                .expect("valid Google response");
        assert_eq!(google.data.translations[0].translated_text, "你好");

        let youdao: YoudaoResponse =
            serde_json::from_str(r#"{"errorCode":"0","translation":["hello"]}"#)
                .expect("valid Youdao response");
        assert_eq!(parse_youdao_response(youdao).unwrap(), "hello");

        let authentication_error: YoudaoResponse =
            serde_json::from_str(r#"{"errorCode":"202"}"#).expect("valid Youdao error response");
        assert!(parse_youdao_response(authentication_error)
            .unwrap_err()
            .contains("鉴权失败"));
    }
}
