use core::ops::Range;
use liner::{Completer, Context};
use num_traits::{One, Zero};
use ramp::{rational::Rational, Int};
use std::fmt::Display;
use std::str::FromStr;
use termion::color;
use Token::*;

// Readable tokens from command line
#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    Number(Rational),
    Minus,
    Plus,
    Times,
    Divide,
    Exp,
    And,
    Or,
    Duplicate,
    Drop,
    Empty,
}

pub struct TokenError {
    pub message: Box<dyn Display>,
    pub span: Range<usize>,
}

fn unexpected_trailing_chars(
    from: &'_ str,
    token: Token,
    size: usize,
) -> Result<Token, TokenError> {
    if from.len() == size {
        Ok(token)
    } else {
        Err(TokenError {
            message: Box::new("Unexpected trailing characters"),
            span: size..(from.len()),
        })
    }
}

impl FromStr for Token {
    type Err = TokenError;
    fn from_str(from: &'_ str) -> Result<Self, Self::Err> {
        let mut chars = from.chars();
        match chars.next().ok_or(TokenError {
            message: Box::new("unexpected empty token"),
            span: 0..0,
        })? {
            '%' => unexpected_trailing_chars(from, Empty, 1),
            '!' => unexpected_trailing_chars(from, Drop, 1),
            '<' => unexpected_trailing_chars(from, Duplicate, 1),
            '^' => unexpected_trailing_chars(from, Exp, 1),
            '/' => unexpected_trailing_chars(from, Divide, 1),
            '*' => unexpected_trailing_chars(from, Times, 1),
            '+' => unexpected_trailing_chars(from, Plus, 1),
            '-' => unexpected_trailing_chars(from, Minus, 1),
            '|' => unexpected_trailing_chars(from, Or, 1),
            '&' => unexpected_trailing_chars(from, And, 1),
            '0' => match chars.next() {
                Some('x') => match Int::from_str_radix(&from[2..], 16) {
                    Ok(n) => Ok(Number(n.into())),
                    Err(e) => Err(TokenError {
                        message: Box::new(e),
                        span: 2..from.len(),
                    }),
                },
                Some('b') => match Int::from_str_radix(&from[2..], 2) {
                    Ok(n) => Ok(Number(n.into())),
                    Err(e) => Err(TokenError {
                        message: Box::new(e),
                        span: 2..from.len(),
                    }),
                },
                _ => match Int::from_str_radix(from, 10) {
                    Ok(n) => Ok(Number(n.into())),
                    Err(e) => Err(TokenError {
                        message: Box::new(e),
                        span: 0..from.len(),
                    }),
                },
            },
            c if c.is_ascii_digit() => match Int::from_str_radix(from, 10) {
                Ok(n) => Ok(Number(n.into())),
                Err(e) => Err(TokenError {
                    message: Box::new(e),
                    span: 0..from.len(),
                }),
            },
            _ => Err(TokenError {
                message: Box::new("unexpected token"),
                span: 0..from.len(),
            }),
        }
    }
}

fn subslice_offset(slice: &str, sub: &str) -> Option<usize> {
    let self_begin = slice.as_ptr() as usize;
    let inner = sub.as_ptr() as usize;
    if inner < self_begin || inner > self_begin.wrapping_add(slice.len()) {
        None
    } else {
        Some(inner.wrapping_sub(self_begin))
    }
}

impl Token {
    pub fn lex(from: &'_ str) -> impl Iterator<Item = Result<Token, <Token as FromStr>::Err>> + '_ {
        from.split_whitespace().map(move |s| {
            // Note: This is a safe unwrap, as the subslice_offset function only returns
            // None when s is not a subslice of from. This can't happen.
            let offset = subslice_offset(from, s).unwrap();
            Token::from_str(s).map_err(|e| TokenError {
                span: (e.span.start + offset)..(e.span.end + offset),
                ..e
            })
        })
    }
}

#[derive(Default)]
pub struct Calculator {
    stack: Vec<Rational>,
}

impl Calculator {
    // Parse a line into tokens and compute them
    pub fn parse(&mut self, word: &str) -> Result<(), TokenError> {
        let tokens = Token::lex(word).collect::<Result<Vec<_>, _>>()?;
        // We check for stack exhaustion before attempting to run anything.
        // that way we don't end up with a half-evaluated expression.
        self.stack_exhaustion(&tokens)
            .map_err(|message| TokenError {
                message,
                span: 0..word.len(),
            })?;
        self.compute(tokens).map_err(|message| TokenError {
            message,
            span: 0..word.len(),
        })?;
        for num in &mut self.stack {
            num.normalize();
        }
        Ok(())
    }

    fn compute(&mut self, tokens: impl IntoIterator<Item = Token>) -> Result<(), Box<dyn Display>> {
        for token in tokens.into_iter() {
            match token {
                Duplicate => {
                    if let Some(mut num) = self.stack.pop() {
                        num.normalize();
                        self.stack.push(num.clone());
                        self.stack.push(num);
                    } else {
                        return Err(Box::new("Incomplete expression, dropped stack"));
                    }
                }
                Empty => self.stack.clear(),
                Drop => {
                    self.stack.pop();
                }
                Number(n) => self.stack.push(n),
                Plus => {
                    let rhs = self.stack.pop();
                    let lhs = self.stack.pop();
                    if let (Some(lhs), Some(rhs)) = (lhs, rhs) {
                        self.stack.push(lhs + rhs);
                    }
                }
                Minus => {
                    let rhs = self.stack.pop();
                    let lhs = self.stack.pop();
                    if let (Some(lhs), Some(rhs)) = (lhs, rhs) {
                        self.stack.push(lhs - rhs);
                    }
                }
                Times => {
                    let rhs = self.stack.pop();
                    let lhs = self.stack.pop();
                    if let (Some(lhs), Some(rhs)) = (lhs, rhs) {
                        self.stack.push(lhs * rhs);
                    }
                }
                Divide => {
                    let rhs = self.stack.pop();
                    let lhs = self.stack.pop();
                    if let (Some(lhs), Some(rhs)) = (lhs, rhs) {
                        self.stack.push(if rhs.is_zero() {
                            Rational::new(0.into(), 1.into())
                        } else {
                            lhs / rhs
                        });
                    }
                }
                Exp => {
                    let rhs = self.stack.pop();
                    let lhs = self.stack.pop();
                    if let (Some(lhs), Some(rhs)) = (lhs, rhs) {
                        self.stack.push(lhs / rhs);
                    }
                }
                And => {
                    let rhs = self.stack.pop();
                    let lhs = self.stack.pop();
                    if let (Some(lhs), Some(rhs)) = (lhs, rhs) {
                        self.stack.push(Rational::new(lhs.round() & rhs.round(), 1.into()));
                    }
                }
                Or => {
                    let rhs = self.stack.pop();
                    let lhs = self.stack.pop();
                    if let (Some(lhs), Some(rhs)) = (lhs, rhs) {
                        self.stack.push(Rational::new(lhs.round() | rhs.round(), 1.into()));
                    }
                }
            }
        }
        Ok(())
    }

    fn stack_exhaustion<'a>(
        &self,
        stack: impl IntoIterator<Item = &'a Token>,
    ) -> Result<(), Box<dyn Display>> {
        let delta = stack
            .into_iter()
            .fold(Some(self.stack.len()), |delta, token| match token {
                // ( -- a)
                Number(_) | Duplicate => delta.map(|d| d + 1),
                // This weirdness, the subtract and then add, is an exact representation of the stack affects
                // of all of these operators, (a b -- c). We must be able to pop 2 off the stack, but we later
                // add 1 back.
                Plus | Minus | Times | Divide | Exp | Or | And => {
                    delta.and_then(|d| d.checked_sub(2)).map(|d| d + 1)
                }
                // (a --)
                Drop => delta.and_then(|d| d.checked_sub(1)),
                Empty => Some(0),
            });
        match delta {
            Some(_) => Ok(()),
            None => Err(Box::new(
                "Stack exhaustion would have occured during evaluation; aborting",
            )),
        }
    }
}

/// This completer does nothing.
///
/// Completion is not really helpful when you have no variables and
/// all tokens are nearly 1 character.
struct EmptyCompleter;

impl Completer for EmptyCompleter {
    fn completions(&mut self, _start: &str) -> Vec<String> {
        Vec::new()
    }
}

/// Colorize errors red
fn colorize(word: &str) -> String {
    let mut res = String::with_capacity(word.len());
    let mut last = 0;
    for token in Token::lex(&word) {
        if let Err(te) = token {
            res.push_str(&word[last..te.span.start]);
            res.push_str(color::LightRed.fg_str());
            res.push_str(&word[te.span.clone()]);
            res.push_str(color::Reset.fg_str());
            last = te.span.end;
        }
    }
    res.push_str(&word[last..]);
    res
}

fn main() {
    let mut calculator = Calculator::default();
    let mut con = Context::new();
    let prefix = color::Fg(color::Magenta);
    let suffix = color::Fg(color::Reset);
    let prompt = format!("{prefix}>>{suffix} ", prefix = prefix, suffix = suffix);
    while let Ok(input) = con.read_line(&prompt, Some(Box::new(colorize)), &mut EmptyCompleter) {
        match calculator.parse(&input) {
            Ok(()) => (),
            Err(TokenError { message, span }) => eprintln!(
                "{}{}{} {}{}",
                " ".repeat(span.start + 3),
                color::LightRed.fg_str(),
                "^".repeat(span.len()),
                message,
                color::Reset.fg_str(),
            ),
        }
        for num in &calculator.stack {
            let (num, den) = num.clone().into_parts();
            if den.is_one() {
                println!("{num} (0x{num:x})", num = num);
            } else {
                println!("{num}/{den} (0x{num:x}/{den:x})", num = num, den = den,);
            }
        }
        con.history.push(input.into()).unwrap();
    }
}
