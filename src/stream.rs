use std::iter::Peekable;
use std::str::Chars;
use std::iter::IntoIterator;
use std::iter::Inspect;

use itertools::Itertools;

#[macro_export]
macro_rules! token_use {
    () => {
        use stream::Token::*;
        use stream::Literal::*;
        use stream::Group::*;
        use stream::GBegin;
        use stream::GEnd;
    }
}

#[derive(PartialEq, Debug)]
pub struct TokenParser {
    pattern_source: String,
    pub tokens: Vec<Token>,
}

#[derive(PartialEq, Debug)]
pub enum Token {
    Grouping(Group),
    Literal(Literal)
}

#[derive(PartialEq, Debug)]
pub enum Literal {
    Char(char),
    AnyChar,
    // One or both of these could be a collating element?
    // I have no idea how to represent/obtain collation stuff
    Range { begin: char, end: char },
    StartOfLine,
    EndOfLine,
    // Character Classes
    Whitespace,
    Alnum,
    Alpha,
    Blank,
    Cntrl,
    Digit,
    Graph,
    Lower,
    Print,
    Punct,
    Upper,
    XDigit,
}

#[derive(PartialEq, Debug)]
pub enum Group {
    Begin(GBegin),
    End(GEnd),
    OrDelimiter,
}
#[derive(PartialEq, Debug)]
pub enum GBegin {
    Pat,
    Cap{ name : Option<String> }, // Capture group
    Not,
    Maybe, // ?
    Always, // +
    Any, // *
    Or,
}
#[derive(PartialEq, Debug)]
pub enum GEnd {
    Pat,
    Cap,
    Not,
    Maybe,
    Always,
    Any,
    Or,
}

const ESCAPABLE_CHARS: &'static [char] = &[
    '.', '+', '?', '*', '(', ')', '[', ']', '|', '^', '$', '\\'
];

impl TokenParser {
    fn read_cap_name(chars: &mut Peekable<Chars>)
        -> Result<String, String> {
        chars.next(); // Consume '<'

        let name: String = chars.by_ref()
            .peeking_take_while( |&c| c != '>')
            .collect();

        match chars.next() {
            Some(c) => {
                assert_eq!(c, '>');
                Ok(name)
            }
            None => Err("Pattern ended in middle of capture group name".to_string()),
        }
    }

    fn prev_object(toks: &[Token]) -> usize {
        token_use!();
        let mut groups = 0;
        for tok in toks.iter().enumerate().rev() {
            match tok.1 {
                &Grouping(End(_)) => groups +=1,
                &Grouping(Begin(_)) => {
                    groups -= 1;
                    if groups == 0 {
                        return tok.0;
                    }
                },
                _ if groups == 0 => return tok.0,
                _ => continue,
            }
        }
        0
    }

    fn parse_char_class(chars: &mut Peekable<Chars>) -> Result<Token, String> {
        use stream::Token::Literal;
        use stream::Literal::*;
        // Assume that the opening '[' was already consumed
        // Assume that we know that the next char is ':'
        chars.next();

        let mut class = String::new();

        while let Some(&c) = chars.peek() {
            chars.next();
            match c {
                ':' => {
                    if Some(&']') == chars.peek() {
                        chars.next();
                        break;
                    } else {
                        class.push(c);
                    }
                },
                _ => class.push(c),
            }
        }

        if None == chars.peek() {
            return Err("Pattern ended in middle of character class".to_string());
        }

        match class.as_ref() {
            "alnum" => Ok(Literal(Alnum)),
            "alpha" => Ok(Literal(Alpha)),
            "blank" => Ok(Literal(Blank)),
            "cntrl" => Ok(Literal(Cntrl)),
            "digit" => Ok(Literal(Digit)),
            "graph" => Ok(Literal(Graph)),
            "lower" => Ok(Literal(Lower)),
            "print" => Ok(Literal(Print)),
            "punct" => Ok(Literal(Punct)),
            "space" => Ok(Literal(Whitespace)),
            "upper" => Ok(Literal(Upper)),
            "xdigit" => Ok(Literal(XDigit)),
            _ => Err("Locale-specific character classes not supported".to_string()),
        }
    }

    fn parse_bracket(chars: &mut Peekable<Chars>) -> Result<Vec<Token>, String> {
        token_use!();
        let mut toks = if Some(&'^') == chars.peek() {
            chars.next();
            vec![Grouping(Begin(GBegin::Not)), Grouping(Begin(GBegin::Or))]
        } else {
            vec![Grouping(Begin(GBegin::Or))]
        };

        if Some(&']') == chars.peek() {
            chars.next();

            toks.push(Literal(Char(']')));
            toks.push(Grouping(OrDelimiter));
        }

        while let Some(c) = chars.next() {
            match c {
                '[' => {
                    if let Some(&next_char) = chars.peek() {
                        match next_char {
                            '.' => return Err("Collating elements not supported".to_string()),
                            '=' => return Err("Equivalence classes not supported".to_string()),
                            ':' => {
                                let class_tok = TokenParser::parse_char_class(chars)?;
                                toks.push(class_tok);
                                toks.push(Grouping(OrDelimiter));
                            },
                            _ => {
                                toks.push(Literal(Char(c)));
                                toks.push(Grouping(OrDelimiter));
                            },
                        }
                    }
                    // Do nothing on `else`, since we'll return Err after `while`
                },
                ']' => {
                    // Remove trailing Or
                    toks.pop();

                    toks.push(Grouping(End(GEnd::Or)));
                    if let Some(&Grouping(Begin(GBegin::Not))) = toks.first() {
                        toks.push(Grouping(End(GEnd::Not)));
                    }
                    return Ok(toks);
                },
                '-' => {
                    // If we have something before this and there is more after
                    if Some(&Grouping(OrDelimiter)) == toks.last()
                        && Some(&']') != chars.peek() {

                        if let Some(next_char) = chars.next() {
                            let next_is_period = Some(&'.') == chars.peek();

                            match next_char {
                                '[' if next_is_period => {
                                    return Err("Collating elements not supported".to_string());
                                },
                                _ => {
                                    // Remove OrDelimiter
                                    toks.pop();
                                    if let Some(Literal(Char(first))) = toks.pop() {
                                        toks.push(Literal(Range {
                                            begin: first,
                                            end: next_char,
                                        }));
                                        toks.push(Grouping(OrDelimiter));
                                    } else {
                                        return Err("Something went wrong on our end".to_string());
                                    }
                                },
                            }
                        } else {
                            // Return an Err
                            break;
                        }

                // If the '-' is the first or last thing in bracket expression
                    } else {
                        // Use it as the literal char
                        toks.push(Literal(Char(c)));
                        toks.push(Grouping(OrDelimiter));
                    }
                },
                _ => {
                    toks.push(Literal(Char(c)));
                    toks.push(Grouping(OrDelimiter));
                },

            }
        }

        return Err("Pattern ended in middle of bracket expression".to_string());
    }

    pub fn from_string(pat: &str) -> Result<TokenParser, String> {
        token_use!();

        let mut open_groups: Vec<usize> = Vec::new();
        let mut toks: Vec<Token> = vec![ Grouping(Begin(GBegin::Pat)) ];

        let mut chars = pat.chars().peekable();


        'source_loop: while let Some(c) = chars.next() {
            match c {
                '(' => {
                    if chars.peek() == Some(&':') {
                        chars.next();
                        if let Some(&next_char) = chars.peek() {
                            match next_char {
                                '<' => {
                                    let name = TokenParser::read_cap_name(&mut chars)?;
                                    let name = Some(name);
                                    toks.push(Grouping(Begin(GBegin::Cap { name: name })));
                                    open_groups.push(toks.len() - 1);
                                },
                                _ =>
                                    return Err(format!("Special capture group (:{} not implented", next_char)),
                            }
                        } else {
                            return Err("Unclosed capture group".to_string());
                        }

                    } else {
                        toks.push(Grouping(Begin(GBegin::Cap { name: None })));
                        open_groups.push(toks.len() - 1);
                    }
                },
                ')' => {
                    let mut index = open_groups.pop().ok_or("Group closed without being opened".to_string())?;

                    if let &Grouping(Begin(GBegin::Or)) = &toks[index] {
                        toks.push(Grouping(End(GEnd::Or)));
                        index = open_groups.pop().ok_or("Group closed without being opened".to_string())?;
                    }
                    let index = index;

                    if let &Grouping(Begin(GBegin::Cap {..})) = &toks[index] {
                        toks.push(Grouping(End(GEnd::Cap)));
                    } else {
                        return Err("Closed group when other things are open".to_string());
                    }

                },
                '?' => {
                    let index = TokenParser::prev_object(&mut toks);
                    toks.insert(index, Grouping(Begin(GBegin::Maybe)));
                    toks.push(Grouping(End(GEnd::Maybe)));
                },
                '+' => {
                    let index = TokenParser::prev_object(&mut toks);
                    toks.insert(index, Grouping(Begin(GBegin::Always)));
                    toks.push(Grouping(End(GEnd::Always)));
                },
                '*' => {
                    let index = TokenParser::prev_object(&mut toks);
                    toks.insert(index, Grouping(Begin(GBegin::Any)));
                    toks.push(Grouping(End(GEnd::Any)));
                },
                '|' => {
                    toks.push(Grouping(OrDelimiter));

                    let index = if let Some(&index) = open_groups.last() {
                        match toks[index] {
                            Grouping(Begin(GBegin::Cap {..})) => index + 1,
                            Grouping(Begin(GBegin::Or)) => continue 'source_loop,
                            _ => 1,
                        }
                    } else {
                        1
                    };
                    let index = index;
                    toks.insert(index, Grouping(Begin(GBegin::Or)));
                    open_groups.push(index);
                },
                '[' => {
                    let range_toks = TokenParser::parse_bracket(&mut chars)?;
                    toks.extend(range_toks);
                },
                '^' => toks.push(Literal(StartOfLine)),
                '$' => toks.push(Literal(EndOfLine)),
                '.' => toks.push(Literal(AnyChar)),
                '\\' => {
                    if let Some(next_char) = chars.next() {
                        if ESCAPABLE_CHARS.contains(&next_char) {
                            toks.push(Literal(Char(next_char)));
                        } else {
                            match next_char {
                                'w' => toks.push(Literal(Alnum)), //word
                                'W' => {
                                    toks.push(Grouping(Begin(GBegin::Not)));
                                    toks.push(Literal(Alnum));
                                    toks.push(Grouping(End(GEnd::Not)));
                                }, //not word
                                'd' => toks.push(Literal(Digit)), //digit
                                'D' => {
                                    toks.push(Grouping(Begin(GBegin::Not)));
                                    toks.push(Literal(Digit));
                                    toks.push(Grouping(End(GEnd::Not)));
                                }, //not digit
                                's' => toks.push(Literal(Whitespace)), //Whitespace
                                'S' => {
                                    toks.push(Grouping(Begin(GBegin::Not)));
                                    toks.push(Literal(Whitespace));
                                    toks.push(Grouping(End(GEnd::Not)));
                                }, //not Whitespace
                                _ => return Err(format!("Character class {} not implemented", next_char)),
                            }
                        }
                    } else {
                        return Err("Pattern ended when expecting escaped character".to_string());
                    }
                },
                _ => toks.push(Literal(Char(c))),
            }
        }

        if let Some(&index) = open_groups.last() {
            println!("Last open group = {:?}", toks[index]);
            match toks[index] {
                Grouping(Begin(GBegin::Or)) => {
                    toks.push(Grouping(End(GEnd::Or)));
                    open_groups.pop();
                },
                _ => (),
            }
        }

        if open_groups.is_empty() {
            toks.push(Grouping(End(GEnd::Pat)));
            let parser = TokenParser {
                pattern_source: pat.to_string(),
                tokens: toks,
            };

            Ok(parser)
        } else {
            Err("Pattern ended with unclosed groups".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    mod basic {
        #[test]
        fn basic_literal() {
            use stream;
            token_use!();
            let pat = "abcd";
            let expected_toks = vec![Grouping(Begin(GBegin::Pat)),
                Literal(Char('a')), Literal(Char('b')),
                Literal(Char('c')), Literal(Char('d')),
                Grouping(End(GEnd::Pat))];

            let parser = stream::TokenParser::from_string(pat).unwrap();
            assert_eq!(expected_toks, parser.tokens);
        }

        #[test]
        fn escape_special_chars() {
            use stream;
            token_use!();

            for escapable_char in stream::ESCAPABLE_CHARS {
                let pat_begin = r"ab\";
                let pat_end = "cd";
                let expected_toks_begin = vec![Grouping(Begin(GBegin::Pat)),
                    Literal(Char('a')), Literal(Char('b'))];
                let expected_toks_end = vec![ Literal(Char('c')),
                    Literal(Char('d')), Grouping(End(GEnd::Pat))];

                let mut pattern = String::new();
                pattern.push_str(pat_begin);
                pattern.push(*escapable_char);
                pattern.push_str(pat_end);
                let pattern = pattern;
                let par = stream::TokenParser::from_string(&pattern).unwrap();

                let mut expected_toks: Vec<stream::Token> = Vec::new();
                expected_toks.extend(expected_toks_begin);
                expected_toks.push(Literal(Char(*escapable_char)));
                expected_toks.extend(expected_toks_end);
                let expected_toks = expected_toks;

                assert_eq!(expected_toks, par.tokens);
            }
        }

        #[test]
        fn prev_object() {
            use stream;
            token_use!();
            let toks = vec![Grouping(Begin(GBegin::Pat)),
                Literal(Char('a')), Literal(Char('b')),
                Literal(Char('c'))];

            let index = stream::TokenParser::prev_object(&toks);
            assert_eq!(3, index);
        }
    }

    mod capture_group {
        #[test]
        fn name_match() {
            use stream;
            let s = "(:<A NAME>asdf)";
            let mut chars = s.chars().peekable();
            assert_eq!('(',chars.next().unwrap());
            assert_eq!(':', chars.next().unwrap());
            let name = stream::TokenParser::read_cap_name(&mut chars).unwrap();
            assert_eq!("A NAME", name);
            let remain: String = chars.collect();
            assert_eq!("asdf)", remain);
        }

        #[test]
        fn name_no_close() {
            use stream;
            let s = "(:<A NAMEasdf)";
            let mut chars = s.chars().peekable();
            assert_eq!('(', chars.next().unwrap());
            assert_eq!(':', chars.next().unwrap());
            let name = stream::TokenParser::read_cap_name(&mut chars);
            assert!(name.is_err());
            assert_eq!(0, chars.count());
        }

        #[test]
        fn literal_unnamed() {
            use stream;
            token_use!();
            let pat = "a(bc)d";
            let expected_toks = vec![Grouping(Begin(GBegin::Pat)),
                Literal(Char('a')), Grouping(Begin(GBegin::Cap {name: None})),
                Literal(Char('b')), Literal(Char('c')),
                Grouping(End(GEnd::Cap)), Literal(Char('d')),
                Grouping(End(GEnd::Pat))];

            let name = stream::TokenParser::from_string(pat).unwrap();
            assert_eq!(expected_toks, name.tokens);


        }

        #[test]
        fn literal_named() {
            use stream;
            token_use!();
            let pat = "a(:<A GROUP>bc)d";
            let expected_toks = vec![Grouping(Begin(GBegin::Pat)),
                Literal(Char('a')), Grouping(Begin(GBegin::Cap {name: Some("A GROUP".to_string())})),
                Literal(Char('b')), Literal(Char('c')),
                Grouping(End(GEnd::Cap)), Literal(Char('d')),
                Grouping(End(GEnd::Pat))];

            let name = stream::TokenParser::from_string(pat).unwrap();
            assert_eq!(expected_toks, name.tokens);
        }
    }

    mod quantifier {
        #[test]
        fn maybe_literal() {
            use stream;
            token_use!();
            let pat = "abc?d";
            let expected_toks = vec![Grouping(Begin(GBegin::Pat)),
                Literal(Char('a')), Literal(Char('b')),
                Grouping(Begin(GBegin::Maybe)), Literal(Char('c')),
                Grouping(End(GEnd::Maybe)), Literal(Char('d')),
                Grouping(End(GEnd::Pat))];

            let name = stream::TokenParser::from_string(pat).unwrap();
            assert_eq!(expected_toks, name.tokens);
        }

        #[test]
        fn maybe_capture_group() {
            use stream;
            token_use!();
            let pat = "a(bc)?d";
            let expected_toks = vec![Grouping(Begin(GBegin::Pat)),
                Literal(Char('a')), Grouping(Begin(GBegin::Maybe)),
                Grouping(Begin(GBegin::Cap {name: None})), Literal(Char('b')),
                Literal(Char('c')), Grouping(End(GEnd::Cap)),
                Grouping(End(GEnd::Maybe)), Literal(Char('d')),
                Grouping(End(GEnd::Pat))];

            let name = stream::TokenParser::from_string(pat).unwrap();
            assert_eq!(expected_toks, name.tokens);

        }
        #[test]
        fn always_literal() {
            use stream;
            token_use!();
            let pat = "abc+d";
            let expected_toks = vec![Grouping(Begin(GBegin::Pat)),
                Literal(Char('a')), Literal(Char('b')),
                Grouping(Begin(GBegin::Always)), Literal(Char('c')),
                Grouping(End(GEnd::Always)), Literal(Char('d')),
                Grouping(End(GEnd::Pat))];

            let name = stream::TokenParser::from_string(pat).unwrap();
            assert_eq!(expected_toks, name.tokens);
        }

        #[test]
        fn always_capture_group() {
            use stream;
            token_use!();
            let pat = "a(bc)+d";
            let expected_toks = vec![Grouping(Begin(GBegin::Pat)),
                Literal(Char('a')), Grouping(Begin(GBegin::Always)),
                Grouping(Begin(GBegin::Cap {name: None})), Literal(Char('b')),
                Literal(Char('c')), Grouping(End(GEnd::Cap)),
                Grouping(End(GEnd::Always)), Literal(Char('d')),
                Grouping(End(GEnd::Pat))];

            let name = stream::TokenParser::from_string(pat).unwrap();
            assert_eq!(expected_toks, name.tokens);

        }
        #[test]
        fn any_literal() {
            use stream;
            token_use!();
            let pat = "abc*d";
            let expected_toks = vec![Grouping(Begin(GBegin::Pat)),
                Literal(Char('a')), Literal(Char('b')),
                Grouping(Begin(GBegin::Any)), Literal(Char('c')),
                Grouping(End(GEnd::Any)), Literal(Char('d')),
                Grouping(End(GEnd::Pat))];

            let name = stream::TokenParser::from_string(pat).unwrap();
            assert_eq!(expected_toks, name.tokens);
        }

        #[test]
        fn any_capture_group() {
            use stream;
            token_use!();
            let pat = "a(bc)*d";
            let expected_toks = vec![Grouping(Begin(GBegin::Pat)),
                Literal(Char('a')), Grouping(Begin(GBegin::Any)),
                Grouping(Begin(GBegin::Cap {name: None})), Literal(Char('b')),
                Literal(Char('c')), Grouping(End(GEnd::Cap)),
                Grouping(End(GEnd::Any)), Literal(Char('d')),
                Grouping(End(GEnd::Pat))];

            let name = stream::TokenParser::from_string(pat).unwrap();
            assert_eq!(expected_toks, name.tokens);
        }
    }

    mod or {
        #[test]
        fn single_without_group() {
            use stream;
            token_use!();
            let pat = "abc|d";
            let expected_toks = vec![Grouping(Begin(GBegin::Pat)),
                Grouping(Begin(GBegin::Or)), Literal(Char('a')),
                Literal(Char('b')), Literal(Char('c')),
                Grouping(OrDelimiter), Literal(Char('d')),
                Grouping(End(GEnd::Or)), Grouping(End(GEnd::Pat))];

            let name = stream::TokenParser::from_string(pat).unwrap();
            assert_eq!(expected_toks, name.tokens);
        }

        #[test]
        fn multiple_without_group() {
            use stream;
            token_use!();
            let pat = "a|b|c|d";
            let expected_toks = vec![Grouping(Begin(GBegin::Pat)),
                Grouping(Begin(GBegin::Or)), Literal(Char('a')),
                Grouping(OrDelimiter), Literal(Char('b')),
                Grouping(OrDelimiter), Literal(Char('c')),
                Grouping(OrDelimiter), Literal(Char('d')),
                Grouping(End(GEnd::Or)), Grouping(End(GEnd::Pat))];

            let name = stream::TokenParser::from_string(pat).unwrap();
            assert_eq!(expected_toks, name.tokens);
        }

        #[test]
        fn single_with_group() {
            use stream;
            token_use!();
            let pat = "a(b|c)d";
            let expected_toks = vec![Grouping(Begin(GBegin::Pat)),
                Literal(Char('a')), Grouping(Begin(GBegin::Cap {name: None})),
                Grouping(Begin(GBegin::Or)), Literal(Char('b')),
                Grouping(OrDelimiter), Literal(Char('c')),
                Grouping(End(GEnd::Or)), Grouping(End(GEnd::Cap)),
                Literal(Char('d')), Grouping(End(GEnd::Pat))];

            let name = stream::TokenParser::from_string(pat).unwrap();
            assert_eq!(expected_toks, name.tokens);
        }

        #[test]
        fn multiple_with_group() {
            use stream;
            token_use!();
            let pat = "a(b|c|d)";
            let expected_toks = vec![Grouping(Begin(GBegin::Pat)),
                Literal(Char('a')), Grouping(Begin(GBegin::Cap {name: None})),
                Grouping(Begin(GBegin::Or)), Literal(Char('b')),
                Grouping(OrDelimiter), Literal(Char('c')),
                Grouping(OrDelimiter), Literal(Char('d')),
                Grouping(End(GEnd::Or)), Grouping(End(GEnd::Cap)),
                Grouping(End(GEnd::Pat))];

            let name = stream::TokenParser::from_string(pat).unwrap();
            assert_eq!(expected_toks, name.tokens);
        }
    }

    mod bracket {
        #[test]
        fn single_literal() {
            use stream;
            token_use!();

            let pat = "ab[c]d";
            let expected_toks = vec![Grouping(Begin(GBegin::Pat)),
                Literal(Char('a')), Literal(Char('b')),
                Grouping(Begin(GBegin::Or)), Literal(Char('c')),
                Grouping(End(GEnd::Or)), Literal(Char('d')),
                Grouping(End(GEnd::Pat))];

            let name = stream::TokenParser::from_string(pat).unwrap();
            assert_eq!(expected_toks, name.tokens);
        }

        #[test]
        fn multiple_literal() {
            use stream;
            token_use!();

            let pat = "a[bcd]e";
            let expected_toks = vec![Grouping(Begin(GBegin::Pat)),
                Literal(Char('a')), Grouping(Begin(GBegin::Or)),
                Literal(Char('b')), Grouping(OrDelimiter),
                Literal(Char('c')), Grouping(OrDelimiter),
                Literal(Char('d')), Grouping(End(GEnd::Or)),
                Literal(Char('e')), Grouping(End(GEnd::Pat))];

            let name = stream::TokenParser::from_string(pat).unwrap();
            assert_eq!(expected_toks, name.tokens);
        }

        #[test]
        fn not_single_literal() {
            use stream;
            token_use!();

            let pat = "ab[^c]d";
            let expected_toks = vec![Grouping(Begin(GBegin::Pat)),
                Literal(Char('a')), Literal(Char('b')),
                Grouping(Begin(GBegin::Not)), Grouping(Begin(GBegin::Or)),
                Literal(Char('c')), Grouping(End(GEnd::Or)),
                Grouping(End(GEnd::Not)), Literal(Char('d')),
                Grouping(End(GEnd::Pat))];

            let name = stream::TokenParser::from_string(pat).unwrap();
            assert_eq!(expected_toks, name.tokens);
        }

        #[test]
        fn not_multiple_literal() {
            use stream;
            token_use!();

            let pat = "a[^bcd]e";
            let expected_toks = vec![Grouping(Begin(GBegin::Pat)),
                Literal(Char('a')), Grouping(Begin(GBegin::Not)),
                Grouping(Begin(GBegin::Or)), Literal(Char('b')),
                Grouping(OrDelimiter), Literal(Char('c')),
                Grouping(OrDelimiter), Literal(Char('d')),
                Grouping(End(GEnd::Or)), Grouping(End(GEnd::Not)),
                Literal(Char('e')), Grouping(End(GEnd::Pat))];

            let name = stream::TokenParser::from_string(pat).unwrap();
            assert_eq!(expected_toks, name.tokens);
        }

        #[test]
        fn literal_bracket() {
            use stream;
            token_use!();

            let pat = "abcd[]]e";
            let expected_toks = vec![Grouping(Begin(GBegin::Pat)),
                Literal(Char('a')), Literal(Char('b')),
                Literal(Char('c')), Literal(Char('d')),
                Grouping(Begin(GBegin::Or)), Literal(Char(']')),
                Grouping(End(GEnd::Or)), Literal(Char('e')),
                Grouping(End(GEnd::Pat))];

            let name = stream::TokenParser::from_string(pat).unwrap();
            assert_eq!(expected_toks, name.tokens);
        }

        #[test]
        fn not_literal_bracket() {
            use stream;
            token_use!();

            let pat = "abcd[^]]e";
            let expected_toks = vec![Grouping(Begin(GBegin::Pat)),
                Literal(Char('a')), Literal(Char('b')),
                Literal(Char('c')), Literal(Char('d')),
                Grouping(Begin(GBegin::Not)), Grouping(Begin(GBegin::Or)),
                Literal(Char(']')), Grouping(End(GEnd::Or)),
                Grouping(End(GEnd::Not)), Literal(Char('e')),
                Grouping(End(GEnd::Pat))];

            let name = stream::TokenParser::from_string(pat).unwrap();
            assert_eq!(expected_toks, name.tokens);
        }

        #[test]
        fn literal_dash_begin() {
            use stream;
            token_use!();

            let pat = "abc[-d]e";
            let expected_toks = vec![Grouping(Begin(GBegin::Pat)),
                Literal(Char('a')), Literal(Char('b')),
                Literal(Char('c')), Grouping(Begin(GBegin::Or)),
                Literal(Char('-')), Grouping(OrDelimiter),
                Literal(Char('d')), Grouping(End(GEnd::Or)),
                Literal(Char('e')), Grouping(End(GEnd::Pat))];

            let name = stream::TokenParser::from_string(pat).unwrap();
            assert_eq!(expected_toks, name.tokens);
        }
        #[test]
        fn not_literal_dash_begin() {
            use stream;
            token_use!();

            let pat = "abc[^-d]e";
            let expected_toks = vec![Grouping(Begin(GBegin::Pat)),
                Literal(Char('a')), Literal(Char('b')),
                Literal(Char('c')), Grouping(Begin(GBegin::Not)),
                Grouping(Begin(GBegin::Or)), Literal(Char('-')),
                Grouping(OrDelimiter), Literal(Char('d')),
                Grouping(End(GEnd::Or)), Grouping(End(GEnd::Not)),
                Literal(Char('e')), Grouping(End(GEnd::Pat))];

            let name = stream::TokenParser::from_string(pat).unwrap();
            assert_eq!(expected_toks, name.tokens);
        }

        #[test]
        fn literal_dash_end() {
            use stream;
            token_use!();

            let pat = "abc[]d-]e";
            let expected_toks = vec![Grouping(Begin(GBegin::Pat)),
                Literal(Char('a')), Literal(Char('b')),
                Literal(Char('c')), Grouping(Begin(GBegin::Or)),
                Literal(Char(']')), Grouping(OrDelimiter),
                Literal(Char('d')), Grouping(OrDelimiter),
                Literal(Char('-')), Grouping(End(GEnd::Or)),
                Literal(Char('e')), Grouping(End(GEnd::Pat))];

            let name = stream::TokenParser::from_string(pat).unwrap();
            assert_eq!(expected_toks, name.tokens);
        }

        #[test]
        fn not_literal_dash_end() {
            use stream;
            token_use!();

            let pat = "abc[^]d-]e";
            let expected_toks = vec![Grouping(Begin(GBegin::Pat)),
                Literal(Char('a')), Literal(Char('b')),
                Literal(Char('c')), Grouping(Begin(GBegin::Not)),
                Grouping(Begin(GBegin::Or)), Literal(Char(']')),
                Grouping(OrDelimiter), Literal(Char('d')),
                Grouping(OrDelimiter), Literal(Char('-')),
                Grouping(End(GEnd::Or)), Grouping(End(GEnd::Not)),
                Literal(Char('e')), Grouping(End(GEnd::Pat))];

            let name = stream::TokenParser::from_string(pat).unwrap();
            assert_eq!(expected_toks, name.tokens);
        }

        #[test]
        fn char_class_test_all() {
            use stream;
            token_use!();

            let classes  = vec![
                ("alnum", Literal(Alnum)),
                ("alpha", Literal(Alpha)),
                ("blank", Literal(Blank)),
                ("cntrl", Literal(Cntrl)),
                ("digit", Literal(Digit)),
                ("graph", Literal(Graph)),
                ("lower", Literal(Lower)),
                ("print", Literal(Print)),
                ("punct", Literal(Punct)),
                ("space", Literal(Whitespace)),
                ("upper", Literal(Upper)),
                ("xdigit", Literal(XDigit))
            ];

            let pat_begin = "abc[[:";
            let pat_end = ":]]d";


            for (class_name, expected_class) in classes {

                let expected_toks_begin = vec![
                    Grouping(Begin(GBegin::Pat)),
                    Literal(Char('a')),
                    Literal(Char('b')),
                    Literal(Char('c')),
                    Grouping(Begin(GBegin::Or)),

                ];
                let expected_toks_end = vec![
                    Grouping(End(GEnd::Or)),
                    Literal(Char('d')),
                    Grouping(End(GEnd::Pat))
                ];

                let mut pattern = String::new();
                pattern.push_str(pat_begin);
                pattern.push_str(class_name);
                pattern.push_str(pat_end);
                let pattern = pattern;
                let par = stream::TokenParser::from_string(&pattern).unwrap();

                let mut expected_toks: Vec<stream::Token> = Vec::new();
                expected_toks.extend(expected_toks_begin);
                expected_toks.push(expected_class);
                expected_toks.extend(expected_toks_end);
                let expected_toks = expected_toks;

                assert_eq!(expected_toks, par.tokens);

            }
        }

        #[test]
        fn char_class_invalid() {
            use stream;
            let pat = "abc[[:qwer:]]e";

            let err = stream::TokenParser::from_string(pat);
            let expected_err = Err("Locale-specific character classes not supported".to_string());
            assert_eq!(expected_err, err);
        }

        #[test]
        fn collating_symbol() {
            use stream;
            let pat = "abc[[.ch.]]e";

            let err = stream::TokenParser::from_string(pat);
            let expected_err = Err("Collating elements not supported".to_string());
            assert_eq!(expected_err, err);
        }

        #[test]
        fn equivalence_class() {
            use stream;
            let pat = "abc[[=a=]]e";

            let err = stream::TokenParser::from_string(pat);
            let expected_err = Err("Equivalence classes not supported".to_string());
            assert_eq!(expected_err, err);
        }
    }
}
