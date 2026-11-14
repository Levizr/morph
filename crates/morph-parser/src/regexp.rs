//! JavaScript regular-expression literal validation and translation.
//!
//! Morph lowers regex literals to `morph::RegExp`, a `std::wregex` wrapper
//! that matches UTF-16 code units (the same unit JavaScript uses without the
//! `u` flag). This module rewrites the JavaScript-only spellings that
//! `std::regex` handles differently and rejects constructs whose semantics
//! cannot be preserved. `morph check` reports every rejection, so a pattern
//! never silently changes meaning.

/// A regex pattern translated to the `std::regex` ECMAScript dialect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslatedRegex {
    /// UTF-8 pattern text passed to `morph::RegExp`.
    pub pattern: String,
    /// The JavaScript `i` flag, lowered to `std::regex::icase`.
    pub ignore_case: bool,
}

/// A pattern or flag that cannot be translated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegexError {
    pub message: String,
    /// Byte offset into the source pattern (0 for flag errors).
    pub offset: usize,
}

impl RegexError {
    fn new(message: impl Into<String>, offset: usize) -> Self {
        Self { message: message.into(), offset }
    }
}

/// JavaScript `\s` whitespace, spelled out because `std::regex`'s `\s`
/// is ASCII-only (it misses NBSP and the Unicode space separators).
const JS_WHITESPACE: &str = " \t\n\r\u{000b}\u{000c}\u{00a0}\u{1680}\u{2000}\u{2001}\u{2002}\u{2003}\u{2004}\u{2005}\u{2006}\u{2007}\u{2008}\u{2009}\u{200a}\u{2028}\u{2029}\u{202f}\u{205f}\u{3000}\u{feff}";

/// Upper bound for `{m,n}` counts. JavaScript accepts larger values, but
/// std::regex would allocate unbounded storage; reject rather than risk a
/// pathological runtime.
const MAX_QUANTIFIER: u64 = 1_000_000;

/// Translate a JavaScript regex pattern and flags.
///
/// Supported flags: none, or `i`. Every other flag is rejected because its
/// semantics (global/sticky state, multiline anchors, dotAll, Unicode,
/// indices, Unicode sets) are not implemented.
pub fn translate(pattern: &str, flags: &str) -> Result<TranslatedRegex, RegexError> {
    let mut ignore_case = false;
    for (offset, flag) in flags.char_indices() {
        match flag {
            'i' if !ignore_case => ignore_case = true,
            'i' => {
                return Err(RegexError::new("duplicate regular expression flag `i`", offset));
            }
            'g' | 'm' | 's' | 'u' | 'y' | 'd' | 'v' => {
                return Err(RegexError::new(
                    format!(
                        "regular expression flag `{flag}` is not supported (only the `i` flag is implemented)"
                    ),
                    offset,
                ));
            }
            other => {
                return Err(RegexError::new(
                    format!("unknown regular expression flag `{other}`"),
                    offset,
                ));
            }
        }
    }

    let mut scanner = Scanner { src: pattern, pos: 0, out: String::new(), groups: Vec::new() };
    scanner.scan()?;
    Ok(TranslatedRegex { pattern: scanner.out, ignore_case })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GroupKind {
    Capture,
    NonCapture,
    Lookahead,
}

impl GroupKind {
    fn quantifiable(self) -> bool {
        matches!(self, Self::Capture | Self::NonCapture)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QuantifierState {
    None,
    Quantified,
    Lazy,
}

#[derive(Debug, Clone)]
struct Escape {
    /// Translated text appended to the output pattern.
    text: String,
    /// Whether a following `*`/`+`/`?`/`{...}` can apply to this escape.
    quantifiable: bool,
    /// Code unit value when the escape denotes one literal character.
    code: Option<u32>,
    /// Whether the escape can be an endpoint of a character-class range.
    rangeable: bool,
    /// `\d`, `\D`, `\w`, `\W`, or an expanded `\s` (never rangeable).
    class_escape: bool,
}

#[derive(Debug, Clone, Copy)]
struct ClassAtom {
    code: Option<u32>,
    rangeable: bool,
    class_escape: bool,
}

struct Scanner<'a> {
    src: &'a str,
    pos: usize,
    out: String,
    groups: Vec<GroupKind>,
}

impl<'a> Scanner<'a> {
    fn peek(&self) -> Option<char> {
        self.src[self.pos..].chars().next()
    }

    fn peek_second(&self) -> Option<char> {
        let mut chars = self.src[self.pos..].chars();
        chars.next();
        chars.next()
    }

    fn starts_with(&self, text: &str) -> bool {
        self.src[self.pos..].starts_with(text)
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }

    fn err(&self, message: impl Into<String>, offset: usize) -> RegexError {
        RegexError::new(message, offset)
    }

    fn scan(&mut self) -> Result<(), RegexError> {
        let mut can_quantify = false;
        let mut quantifier = QuantifierState::None;
        while let Some(ch) = self.peek() {
            match ch {
                '\\' => {
                    let escape = self.scan_escape(false)?;
                    self.out.push_str(&escape.text);
                    can_quantify = escape.quantifiable;
                    quantifier = QuantifierState::None;
                }
                '[' => {
                    let class = self.scan_class()?;
                    self.out.push_str(&class);
                    can_quantify = true;
                    quantifier = QuantifierState::None;
                }
                '(' => {
                    self.scan_group_open()?;
                    can_quantify = false;
                    quantifier = QuantifierState::None;
                }
                ')' => {
                    let start = self.pos;
                    let Some(kind) = self.groups.pop() else {
                        return Err(self.err("unmatched `)` in regular expression", start));
                    };
                    self.bump();
                    self.out.push(')');
                    can_quantify = kind.quantifiable();
                    quantifier = QuantifierState::None;
                }
                '|' => {
                    self.bump();
                    self.out.push('|');
                    can_quantify = false;
                    quantifier = QuantifierState::None;
                }
                '^' | '$' => {
                    self.bump();
                    self.out.push(ch);
                    can_quantify = false;
                    quantifier = QuantifierState::None;
                }
                '.' => {
                    self.bump();
                    // std::regex `.` excludes only `\n`; JavaScript also
                    // excludes `\r`, U+2028, and U+2029.
                    self.out.push_str("[^\\n\\r\u{2028}\u{2029}]");
                    can_quantify = true;
                    quantifier = QuantifierState::None;
                }
                '*' | '+' => {
                    let start = self.pos;
                    self.bump();
                    if !can_quantify || quantifier != QuantifierState::None {
                        return Err(self.err("quantifier has nothing to repeat", start));
                    }
                    self.out.push(ch);
                    can_quantify = false;
                    quantifier = QuantifierState::Quantified;
                }
                '?' => {
                    let start = self.pos;
                    self.bump();
                    match quantifier {
                        QuantifierState::Quantified => {
                            self.out.push('?');
                            can_quantify = false;
                            quantifier = QuantifierState::Lazy;
                        }
                        QuantifierState::Lazy => {
                            return Err(self.err("quantifier has nothing to repeat", start));
                        }
                        QuantifierState::None => {
                            if !can_quantify {
                                return Err(self.err("quantifier has nothing to repeat", start));
                            }
                            self.out.push('?');
                            can_quantify = false;
                            quantifier = QuantifierState::Quantified;
                        }
                    }
                }
                '{' => {
                    let start = self.pos;
                    let text = self.scan_quantifier()?;
                    if !can_quantify || quantifier != QuantifierState::None {
                        return Err(self.err("quantifier has nothing to repeat", start));
                    }
                    self.out.push_str(&text);
                    can_quantify = false;
                    quantifier = QuantifierState::Quantified;
                }
                '}' => {
                    // A bare `}` is a literal in JavaScript; std::regex may
                    // treat it as a malformed quantifier, so escape it.
                    self.bump();
                    self.out.push_str("\\}");
                    can_quantify = true;
                    quantifier = QuantifierState::None;
                }
                ']' => {
                    // A bare `]` is a literal outside a class in JavaScript.
                    self.bump();
                    self.out.push_str("\\]");
                    can_quantify = true;
                    quantifier = QuantifierState::None;
                }
                '\n' | '\r' | '\u{2028}' | '\u{2029}' => {
                    return Err(self.err(
                        "line terminators must be escaped in a regular expression",
                        self.pos,
                    ));
                }
                other => {
                    self.bump();
                    self.out.push(other);
                    can_quantify = true;
                    quantifier = QuantifierState::None;
                }
            }
        }
        if let Some(kind) = self.groups.last() {
            let message = match kind {
                GroupKind::Capture | GroupKind::NonCapture => "unclosed `(` in regular expression",
                GroupKind::Lookahead => "unclosed lookahead in regular expression",
            };
            return Err(self.err(message, self.pos));
        }
        Ok(())
    }

    fn scan_group_open(&mut self) -> Result<(), RegexError> {
        let start = self.pos;
        if self.starts_with("(?:") {
            self.pos += 3;
            self.out.push_str("(?:");
            self.groups.push(GroupKind::NonCapture);
        } else if self.starts_with("(?=") {
            self.pos += 3;
            self.out.push_str("(?=");
            self.groups.push(GroupKind::Lookahead);
        } else if self.starts_with("(?!") {
            self.pos += 3;
            self.out.push_str("(?!");
            self.groups.push(GroupKind::Lookahead);
        } else if self.starts_with("(?<") {
            return Err(self.err("lookbehind and named capture groups are not supported", start));
        } else if self.starts_with("(?") {
            return Err(self.err("unsupported regular expression group syntax", start));
        } else {
            self.bump();
            self.out.push('(');
            self.groups.push(GroupKind::Capture);
        }
        Ok(())
    }

    fn scan_quantifier(&mut self) -> Result<String, RegexError> {
        let start = self.pos;
        self.bump(); // `{`
        let mut min_digits = String::new();
        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() {
                min_digits.push(ch);
                self.bump();
            } else {
                break;
            }
        }
        if min_digits.is_empty() {
            return Err(self.err(
                "bare `{` is not supported; escape it as `\\{` or use a `{m,n}` quantifier",
                start,
            ));
        }
        let min = min_digits
            .parse::<u64>()
            .map_err(|_| self.err("regular expression quantifier is too large", start))?;
        let mut max = Some(min);
        if self.peek() == Some(',') {
            self.bump();
            let mut max_digits = String::new();
            while let Some(ch) = self.peek() {
                if ch.is_ascii_digit() {
                    max_digits.push(ch);
                    self.bump();
                } else {
                    break;
                }
            }
            if max_digits.is_empty() {
                max = None;
            } else {
                let value = max_digits
                    .parse::<u64>()
                    .map_err(|_| self.err("regular expression quantifier is too large", start))?;
                max = Some(value);
            }
        }
        if self.peek() != Some('}') {
            return Err(self.err(
                "bare `{` is not supported; escape it as `\\{` or use a `{m,n}` quantifier",
                start,
            ));
        }
        self.bump();
        if min > MAX_QUANTIFIER || max.is_some_and(|value| value > MAX_QUANTIFIER) {
            return Err(self.err("regular expression quantifier is too large", start));
        }
        if let Some(max) = max {
            if min > max {
                return Err(self.err("regular expression quantifier range is out of order", start));
            }
        }
        Ok(self.src[start..self.pos].to_string())
    }

    fn scan_escape(&mut self, in_class: bool) -> Result<Escape, RegexError> {
        let start = self.pos;
        self.bump(); // `\`
        let Some(ch) = self.bump() else {
            return Err(self.err("trailing backslash in regular expression", start));
        };
        let atom = |text: String, code: Option<u32>| Escape {
            text,
            quantifiable: true,
            code,
            rangeable: true,
            class_escape: false,
        };
        let class_escape = |text: String| Escape {
            text,
            quantifiable: true,
            code: None,
            rangeable: false,
            class_escape: true,
        };
        match ch {
            'b' if in_class => Ok(atom("\\b".to_string(), Some(0x08))),
            'b' => Ok(Escape {
                text: "\\b".to_string(),
                quantifiable: false,
                code: None,
                rangeable: false,
                class_escape: false,
            }),
            'B' if in_class => Ok(atom("B".to_string(), Some(u32::from(b'B')))),
            'B' => Ok(Escape {
                text: "\\B".to_string(),
                quantifiable: false,
                code: None,
                rangeable: false,
                class_escape: false,
            }),
            'd' | 'D' | 'w' | 'W' => Ok(class_escape(format!("\\{ch}"))),
            's' if in_class => Ok(class_escape(JS_WHITESPACE.to_string())),
            's' => Ok(class_escape(format!("[{JS_WHITESPACE}]"))),
            'S' if in_class => Err(self.err(
                "`\\S` inside a character class is not supported; use an explicit negated class",
                start,
            )),
            'S' => Ok(class_escape(format!("[^{JS_WHITESPACE}]"))),
            'f' => Ok(atom("\\f".to_string(), Some(0x0c))),
            'n' => Ok(atom("\\n".to_string(), Some(0x0a))),
            'r' => Ok(atom("\\r".to_string(), Some(0x0d))),
            't' => Ok(atom("\\t".to_string(), Some(0x09))),
            'v' => Ok(atom("\\v".to_string(), Some(0x0b))),
            '0' => {
                if self.peek().is_some_and(|next| next.is_ascii_digit()) {
                    return Err(self.err("octal escapes are not supported", start));
                }
                Ok(atom("\\0".to_string(), Some(0)))
            }
            '1'..='9' => Err(self.err("backreferences are not supported", start)),
            'x' => {
                let code = self.scan_hex(2, start)?;
                Ok(atom(char::from_u32(code).unwrap_or('\u{fffd}').to_string(), Some(code)))
            }
            'u' => {
                if self.peek() == Some('{') {
                    return Err(
                        self.err("`\\u{...}` escapes require the unsupported `u` flag", start)
                    );
                }
                let code = self.scan_hex(4, start)?;
                if (0xd800..=0xdfff).contains(&code) {
                    return Err(self.err("surrogate `\\uXXXX` escapes are not supported", start));
                }
                Ok(atom(char::from_u32(code).unwrap_or('\u{fffd}').to_string(), Some(code)))
            }
            'c' => {
                let Some(letter) = self.peek() else {
                    return Err(self.err("incomplete control escape", start));
                };
                if !letter.is_ascii_alphabetic() {
                    return Err(self.err("`\\c` must be followed by an ASCII letter", start));
                }
                self.bump();
                let code = u32::from(letter.to_ascii_uppercase()) % 32;
                Ok(atom(char::from_u32(code).unwrap_or('\u{fffd}').to_string(), Some(code)))
            }
            'p' | 'P' => Err(self.err("Unicode property escapes are not supported", start)),
            'k' => Err(self.err("named backreferences are not supported", start)),
            '/' => Ok(atom("/".to_string(), Some(u32::from(b'/')))),
            other if other.is_ascii_alphabetic() => {
                // JavaScript identity escape (`\a` is `a`).
                Ok(atom(other.to_string(), Some(u32::from(other))))
            }
            other if other.is_ascii() => Ok(atom(format!("\\{other}"), Some(u32::from(other)))),
            other if other.is_control() || other.is_whitespace() => {
                Err(self.err("line terminators must be escaped in a regular expression", start))
            }
            other => Ok(atom(other.to_string(), Some(u32::from(other)))),
        }
    }

    fn scan_hex(&mut self, digits: usize, start: usize) -> Result<u32, RegexError> {
        let mut value = 0u32;
        for _ in 0..digits {
            let Some(ch) = self.peek() else {
                return Err(self.err("incomplete hexadecimal escape", start));
            };
            let Some(digit) = ch.to_digit(16) else {
                return Err(self.err("invalid hexadecimal escape", start));
            };
            value = value * 16 + digit;
            self.bump();
        }
        Ok(value)
    }

    fn scan_class(&mut self) -> Result<String, RegexError> {
        let start = self.pos;
        self.bump(); // `[`
        let mut class = String::from("[");
        if self.peek() == Some('^') {
            self.bump();
            class.push('^');
        }
        let mut previous: Option<ClassAtom> = None;
        let mut previous_in_range = false;
        loop {
            let Some(ch) = self.peek() else {
                return Err(self.err("unterminated character class", start));
            };
            if ch == ']' {
                self.bump();
                class.push(']');
                return Ok(class);
            }
            if ch == '\\' {
                let escape = self.scan_escape(true)?;
                class.push_str(&escape.text);
                previous = Some(ClassAtom {
                    code: escape.code,
                    rangeable: escape.rangeable,
                    class_escape: escape.class_escape,
                });
                previous_in_range = false;
                continue;
            }
            if ch == '-' {
                let is_leading = previous.is_none();
                let is_trailing = self.peek_second().is_none_or(|next| next == ']');
                if is_leading || is_trailing || previous_in_range {
                    self.bump();
                    class.push('-');
                    previous = None;
                    previous_in_range = false;
                    continue;
                }
                let Some(previous_atom) = previous.take() else {
                    self.bump();
                    class.push('-');
                    continue;
                };
                if !previous_atom.rangeable || previous_atom.class_escape {
                    return Err(self.err(
                        "character-class ranges with escape classes are not supported",
                        self.pos,
                    ));
                }
                self.bump(); // `-`
                let next_atom = if self.peek() == Some('\\') {
                    let escape = self.scan_escape(true)?;
                    class.push('-');
                    class.push_str(&escape.text);
                    ClassAtom {
                        code: escape.code,
                        rangeable: escape.rangeable,
                        class_escape: escape.class_escape,
                    }
                } else {
                    let Some(next) = self.bump() else {
                        return Err(self.err("unterminated character class", start));
                    };
                    if next == ']' {
                        // `-` immediately before `]` is handled as trailing
                        // above; reaching here means a malformed class.
                        return Err(self.err("invalid character-class range", start));
                    }
                    class.push('-');
                    class.push(next);
                    let astral = next as u32 > 0xffff;
                    ClassAtom {
                        code: if astral { None } else { Some(u32::from(next)) },
                        rangeable: !astral,
                        class_escape: false,
                    }
                };
                if !next_atom.rangeable || next_atom.class_escape {
                    return Err(self.err(
                        "character-class ranges with escape classes are not supported",
                        self.pos,
                    ));
                }
                if let (Some(low), Some(high)) = (previous_atom.code, next_atom.code) {
                    if low > high {
                        return Err(self.err("character-class range is out of order", self.pos));
                    }
                }
                previous = Some(next_atom);
                previous_in_range = true;
                continue;
            }
            self.bump();
            class.push(ch);
            let astral = ch as u32 > 0xffff;
            previous = Some(ClassAtom {
                code: if astral { None } else { Some(u32::from(ch)) },
                rangeable: !astral,
                class_escape: false,
            });
            previous_in_range = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(pattern: &str) -> String {
        translate(pattern, "").map(|regex| regex.pattern).unwrap_or_else(|error| {
            panic!("expected {pattern:?} to translate, got {}", error.message)
        })
    }

    fn err(pattern: &str) -> String {
        translate(pattern, "").err().map(|error| error.message).unwrap_or_default()
    }

    #[test]
    fn dot_escapes_line_terminators() {
        assert_eq!(ok("a.c"), "a[^\\n\\r\u{2028}\u{2029}]c");
    }

    #[test]
    fn whitespace_is_unicode_aware() {
        let translated = ok("\\s");
        assert!(translated.contains('\u{00a0}'), "{translated}");
        assert!(translated.starts_with("[ "), "{translated}");
        assert_eq!(ok("\\S"), format!("[^{JS_WHITESPACE}]"));
        assert!(ok("[\\s]").contains('\u{00a0}'));
        assert!(err("[\\S]").contains("inside a character class"));
    }

    #[test]
    fn escapes_are_translated() {
        assert_eq!(ok("\\u00e9"), "é");
        assert_eq!(ok("\\x41"), "A");
        assert_eq!(ok("\\cA"), "\u{0001}");
        assert_eq!(ok("\\a"), "a");
        assert_eq!(ok("\\/"), "/");
        assert_eq!(ok("\\-"), "\\-");
        assert_eq!(ok("[\\B]"), "[B]");
    }

    #[test]
    fn literals_that_std_regex_rejects_are_escaped() {
        assert_eq!(ok("a}"), "a\\}");
        assert_eq!(ok("]"), "\\]");
        assert_eq!(ok("a\\{"), "a\\{");
    }

    #[test]
    fn unsupported_constructs_are_rejected() {
        assert!(err("(?<=a)b").contains("lookbehind"));
        assert!(err("(?<name>a)").contains("lookbehind"));
        assert!(err("\\p{L}").contains("property"));
        assert!(err("\\1").contains("backreferences"));
        assert!(err("\\k<name>").contains("named backreferences"));
        assert!(err("\\u{1f600}").contains("unsupported"));
        assert!(err("\\uD83D").contains("surrogate"));
        assert!(err("\\01").contains("octal"));
        assert!(err("a{").contains("bare `{`"));
        assert!(err("a{3,2}").contains("out of order"));
        assert!(err("[\\d-a]").contains("escape classes"));
        assert!(err("[z-a]").contains("out of order"));
    }

    #[test]
    fn quantifiers_are_validated() {
        assert_eq!(ok("a*?"), "a*?");
        assert_eq!(ok("a{1,2}?"), "a{1,2}?");
        assert!(err("*a").contains("nothing to repeat"));
        assert!(err("a**").contains("nothing to repeat"));
        assert!(err("a{2}{3}").contains("nothing to repeat"));
        assert!(err("(?=a)*").contains("nothing to repeat"));
        assert!(err("\\b*").contains("nothing to repeat"));
    }

    #[test]
    fn flags_are_limited_to_i() {
        let regex = translate("abc", "i").unwrap_or_else(|error| panic!("{}", error.message));
        assert!(regex.ignore_case);
        assert!(translate("abc", "g").is_err());
        assert!(translate("abc", "u").is_err());
        assert!(translate("abc", "ii").is_err());
    }

    #[test]
    fn astral_and_accented_literals_terminate() {
        assert_eq!(ok("é"), "é");
        assert_eq!(ok("😀"), "😀");
        assert_eq!(ok("^😀+$"), "^😀+$");
        assert_eq!(ok("[😀]"), "[😀]");
    }

    #[test]
    fn classes_keep_js_semantics() {
        assert_eq!(ok("[]"), "[]");
        assert_eq!(ok("[^]"), "[^]");
        assert_eq!(ok("[a-b-c]"), "[a-b-c]");
        assert_eq!(ok("[\\b]"), "[\\b]");
        assert_eq!(ok("[\\0]"), "[\\0]");
        assert_eq!(ok("[[a]"), "[[a]");
    }
}
