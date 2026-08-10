use crate::arguments;

const MAX_POSITIONAL_ARGUMENT_INDEX: usize = 31;

enum Placeholder {
    Query,
    Position(usize),
}

pub fn sample_url(template: &str) -> Result<String, String> {
    let scheme_end = template
        .find("://")
        .ok_or_else(|| "网络搜索 URL 模板必须以 http:// 或 https:// 开头".to_string())?;
    let scheme = &template[..scheme_end];
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return Err("网络搜索 URL 模板必须以 http:// 或 https:// 开头".into());
    }
    let after_scheme = &template[scheme_end + 3..];
    let authority_end = after_scheme
        .find(['/', '?', '#'])
        .unwrap_or(after_scheme.len());
    if after_scheme[..authority_end].contains(['{', '}']) {
        return Err("网络搜索占位符不能出现在域名或认证信息中".into());
    }
    expand_template(template, |_| Ok("test".into())).map(|(url, _)| url)
}

pub fn expand_url(template: &str, arguments: &str) -> Result<String, String> {
    let encoded_query = urlencoding::encode(arguments).into_owned();
    let mut positional_arguments: Option<Vec<String>> = None;
    expand_template(template, |placeholder| match placeholder {
        Placeholder::Query => Ok(encoded_query.clone()),
        Placeholder::Position(index) => {
            if positional_arguments.is_none() {
                positional_arguments = Some(arguments::parse(arguments)?);
            }
            let values = positional_arguments
                .as_ref()
                .expect("positional arguments initialized");
            let value = values
                .get(index)
                .ok_or_else(|| format!("网络搜索参数不足：URL 模板需要第 {} 个参数", index + 1))?;
            Ok(urlencoding::encode(value).into_owned())
        }
    })
    .map(|(url, _)| url)
}

pub fn requires_arguments(template: &str) -> Result<bool, String> {
    expand_template(template, |_| Ok(String::new())).map(|(_, found)| found)
}

fn expand_template<F>(template: &str, mut resolve: F) -> Result<(String, bool), String>
where
    F: FnMut(Placeholder) -> Result<String, String>,
{
    let mut output = String::with_capacity(template.len());
    let mut remaining = template;
    let mut found = false;

    while let Some(open) = remaining.find('{') {
        output.push_str(&remaining[..open]);
        let after_open = &remaining[open + 1..];
        let close = after_open
            .find('}')
            .ok_or_else(|| "网络搜索 URL 模板存在未闭合的占位符".to_string())?;
        let token = &after_open[..close];
        let placeholder = if token == "query" {
            Placeholder::Query
        } else if let Some(index_text) = token.strip_prefix("query") {
            if index_text.is_empty() || !index_text.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(format!(
                    "网络搜索 URL 模板占位符 {{{token}}} 无效，仅支持 {{query}} 或 {{query0}}、{{query1}}…"
                ));
            }
            let index = index_text
                .parse::<usize>()
                .map_err(|_| format!("网络搜索 URL 模板占位符 {{{token}}} 无效"))?;
            if index > MAX_POSITIONAL_ARGUMENT_INDEX {
                return Err(format!(
                    "网络搜索位置参数最大支持 {{query{MAX_POSITIONAL_ARGUMENT_INDEX}}}"
                ));
            }
            Placeholder::Position(index)
        } else {
            return Err(format!(
                "网络搜索 URL 模板占位符 {{{token}}} 无效，仅支持 {{query}} 或 {{query0}}、{{query1}}…"
            ));
        };
        output.push_str(&resolve(placeholder)?);
        found = true;
        remaining = &after_open[close + 1..];
    }

    if remaining.contains('}') {
        return Err("网络搜索 URL 模板存在未配对的 }".into());
    }
    output.push_str(remaining);
    Ok((output, found))
}

#[cfg(test)]
mod tests {
    use super::{expand_url, requires_arguments, sample_url};

    #[test]
    fn expands_full_query_and_positional_arguments() {
        assert_eq!(
            expand_url("https://example.com/?q={query}&again={query}", "codex 中文").unwrap(),
            "https://example.com/?q=codex%20%E4%B8%AD%E6%96%87&again=codex%20%E4%B8%AD%E6%96%87"
        );
        assert_eq!(
            expand_url("https://example.com/?q={query0}&v={query1}", "test  codex").unwrap(),
            "https://example.com/?q=test&v=codex"
        );
        assert_eq!(
            expand_url(
                "https://example.com/?q={query0}&v={query1}",
                r#""hello world" codex"#
            )
            .unwrap(),
            "https://example.com/?q=hello%20world&v=codex"
        );
    }

    #[test]
    fn accepts_a_direct_url_without_query_placeholders() {
        let direct = "https://bytedance.feishu.cn/drive/home/";
        assert_eq!(sample_url(direct).unwrap(), direct);
        assert_eq!(expand_url(direct, "").unwrap(), direct);
        assert!(!requires_arguments(direct).unwrap());
        assert!(requires_arguments("https://example.com/?q={query}").unwrap());
    }

    #[test]
    fn rejects_missing_arguments_and_invalid_placeholders() {
        assert!(expand_url("https://example.com/?q={query0}&v={query1}", "only-one").is_err());
        assert!(sample_url("https://example.com/?q={0}").is_err());
        assert!(sample_url("https://example.com/?q={name}").is_err());
        assert!(sample_url("https://{query}.example.com/search").is_err());
        assert!(sample_url("https:{query}/path").is_err());
        assert!(sample_url(r"https:\\{query}\path").is_err());
    }
}
