pub fn parse(arguments: &str) -> Result<Vec<String>, String> {
    let mut values = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut started = false;
    let mut characters = arguments.chars().peekable();

    while let Some(character) = characters.next() {
        if let Some(active_quote) = quote {
            if character == active_quote {
                quote = None;
            } else if character == '\\' && characters.peek() == Some(&active_quote) {
                current.push(characters.next().expect("peeked quote remains available"));
            } else {
                current.push(character);
            }
            started = true;
        } else if character.is_whitespace() {
            if started {
                values.push(std::mem::take(&mut current));
                started = false;
            }
        } else if matches!(character, '\'' | '"') {
            quote = Some(character);
            started = true;
        } else if character == '\\'
            && characters
                .peek()
                .is_some_and(|next| matches!(next, '\'' | '"'))
        {
            current.push(characters.next().expect("peeked quote remains available"));
            started = true;
        } else {
            current.push(character);
            started = true;
        }
    }
    if quote.is_some() {
        return Err("命令参数中的引号没有闭合".into());
    }
    if started {
        values.push(current);
    }
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn parses_multiple_and_quoted_arguments_without_a_shell() {
        assert_eq!(
            parse(r#"one "two words" three"#).unwrap(),
            ["one", "two words", "three"]
        );
        assert_eq!(parse("123456 +8").unwrap(), ["123456", "+8"]);
        assert!(parse("one \"").is_err());
        assert_eq!(
            parse(r#"C:\tmp\a.txt "D:\two words\b.txt""#).unwrap(),
            [r#"C:\tmp\a.txt"#, r#"D:\two words\b.txt"#]
        );
    }
}
