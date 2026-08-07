pub fn evaluate(input: &str) -> Option<f64> {
    let mut parser = Parser::new(input);
    let value = parser.expression()?;
    parser.skip_whitespace();
    (parser.position == parser.input.len() && value.is_finite()).then_some(value)
}

struct Parser<'a> {
    input: &'a [u8],
    position: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input: input.as_bytes(),
            position: 0,
        }
    }

    fn expression(&mut self) -> Option<f64> {
        let mut value = self.term()?;
        loop {
            self.skip_whitespace();
            if self.consume(b'+') {
                value += self.term()?;
            } else if self.consume(b'-') {
                value -= self.term()?;
            } else {
                return Some(value);
            }
        }
    }

    fn term(&mut self) -> Option<f64> {
        let mut value = self.power()?;
        loop {
            self.skip_whitespace();
            if self.consume(b'*') {
                value *= self.power()?;
            } else if self.consume(b'/') {
                value /= self.power()?;
            } else if self.consume(b'%') {
                value %= self.power()?;
            } else {
                return Some(value);
            }
        }
    }

    fn power(&mut self) -> Option<f64> {
        let value = self.unary()?;
        self.skip_whitespace();
        if self.consume(b'^') {
            Some(value.powf(self.power()?))
        } else {
            Some(value)
        }
    }

    fn unary(&mut self) -> Option<f64> {
        self.skip_whitespace();
        if self.consume(b'+') {
            self.unary()
        } else if self.consume(b'-') {
            Some(-self.unary()?)
        } else {
            self.primary()
        }
    }

    fn primary(&mut self) -> Option<f64> {
        self.skip_whitespace();
        if self.consume(b'(') {
            let value = self.expression()?;
            self.skip_whitespace();
            self.consume(b')').then_some(value)
        } else {
            self.number()
        }
    }

    fn number(&mut self) -> Option<f64> {
        self.skip_whitespace();
        let start = self.position;
        let mut seen_digit = false;
        let mut seen_dot = false;

        while let Some(byte) = self.input.get(self.position).copied() {
            if byte.is_ascii_digit() {
                seen_digit = true;
                self.position += 1;
            } else if byte == b'.' && !seen_dot {
                seen_dot = true;
                self.position += 1;
            } else {
                break;
            }
        }

        if !seen_digit {
            self.position = start;
            return None;
        }

        std::str::from_utf8(&self.input[start..self.position])
            .ok()?
            .parse()
            .ok()
    }

    fn consume(&mut self, expected: u8) -> bool {
        if self.input.get(self.position) == Some(&expected) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    fn skip_whitespace(&mut self) {
        while self
            .input
            .get(self.position)
            .is_some_and(u8::is_ascii_whitespace)
        {
            self.position += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::evaluate;

    #[test]
    fn observes_precedence_and_parentheses() {
        assert_eq!(evaluate("11+1"), Some(12.0));
        assert_eq!(evaluate("2+3*4"), Some(14.0));
        assert_eq!(evaluate("(2+3)*4"), Some(20.0));
    }

    #[test]
    fn rejects_incomplete_or_non_finite_expressions() {
        assert_eq!(evaluate("1+"), None);
        assert_eq!(evaluate("1/0"), None);
        assert_eq!(evaluate("hello"), None);
    }
}
