use std::iter::Peekable;
use std::str::Chars;
use std::iter::IntoIterator;
use std::iter::Inspect;

use itertools::Itertools;

pub struct TokenParser {
    pattern_source: String,
    tokens: Vec<Token>,
}

pub enum Token {
    Grouping(Group),
    Literal(Literal)
}

pub enum Literal {
    Char(char),
    Range { begin: char, end: char },
    Whitespace,
    // TODO: Add the rest
}

pub enum Group {
    Begin(GBegin),
    End(GEnd),
    OrDelimiter,
}
pub enum GBegin {
    Pat,
    Cap{ name : Option<String> }, // Capture group
    Not,
    Maybe, // ?
    Always, // +
    Any, // *
    Or,
}
pub enum GEnd {
    Pat,
    Cap,
    Not,
    Maybe,
    Always,
    Any,
    Or,
}

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

    fn prev_object(toks: &Vec<Token>) -> usize {
        use stream::Token::*;
        use stream::Group::*;
        use stream::GBegin;
        use stream::GEnd;
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
                _ if tok.0 == 0 => return tok.0,
                _ => continue,
            }
        }
        0
    }

    pub fn from_string(pat: &str) -> Result<TokenParser, String> {
        use stream::Token::*;
        use stream::Literal::*;
        use stream::Group::*;
        use stream::GBegin;
        use stream::GEnd;

        let mut open_groups: Vec<usize> = Vec::new();
        let mut toks: Vec<Token> = vec![ Grouping(Begin(GBegin::Pat)) ];

        let mut chars = pat.chars().peekable();


        while let Some(c) = chars.next() {
            match c {
                '(' => {
                    if let Some(&next_char) = chars.peek() {
                        if next_char == ':' {
                            chars.next();
                            if let Some(&next_char) = chars.peek() {
                                match next_char {
                                    '<' => {
                                        let name = TokenParser::read_cap_name(&mut chars)?;
                                        let name = Some(name);
                                        toks.push(Grouping(Begin(GBegin::Cap { name: name })));
                                        open_groups.push(toks.len() - 1);
                                    }
                                    _ => {
                                        return Err(format!("Special capture group (:{} not implented", next_char));
                                    }
                                }
                            } else {
                                return Err("Unclosed capture group".to_string());
                            }

                        } else {
                            toks.push(Grouping(Begin(GBegin::Cap { name: None })));
                            open_groups.push(toks.len() - 1);
                        }
                    }
                },
                ')' => {
                    let index = open_groups.pop().ok_or("Group closed without being opened".to_string())?;
                    // let opening = &toks[index];
                    if let &Grouping(Begin(GBegin::Cap {..})) = &toks[index] {
                        toks.push(Grouping(End(GEnd::Cap)));
                    } else {
                        return Err("Closed group when other things are open".to_string());
                    }

                }
                '?' => {
                    toks.push(Grouping(End(GEnd::Maybe)));
                    let index = TokenParser::prev_object(&mut toks);
                    toks.insert(index, Grouping(Begin(GBegin::Maybe)));

                }
                _ => toks.push(Literal(Char(c))),
            }
        }

        Err("h".to_string())
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn cap_name_match() {
        use stream;
        let s = "(:<A NAME>asdf)";
        let mut chars = s.chars().peekable();
        assert_eq!(chars.next().unwrap(), '(');
        assert_eq!(chars.next().unwrap(), ':');
        let name = stream::TokenParser::read_cap_name(&mut chars).unwrap();
        assert_eq!(name, "A NAME");
        let remain: String = chars.collect();
        assert_eq!(remain, "asdf)");
    }

    #[test]
    fn cap_name_no_close() {
        use stream;
        let s = "(:<A NAMEasdf)";
        let mut chars = s.chars().peekable();
        assert_eq!(chars.next().unwrap(), '(');
        assert_eq!(chars.next().unwrap(), ':');
        let name = stream::TokenParser::read_cap_name(&mut chars);
        assert!(name.is_err());
        assert_eq!(chars.count(), 0);
    }
}
