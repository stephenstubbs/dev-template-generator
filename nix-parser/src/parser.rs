use crate::ast::*;
use nom::{
    branch::alt,
    bytes::complete::{tag, take_until, take_while, take_while1},
    character::complete::{alpha1, char, digit1, multispace1},
    combinator::{map, opt, recognize, value},
    multi::{many0, many1, separated_list0, separated_list1},
    sequence::{delimited, pair, preceded, separated_pair, terminated, tuple},
    IResult,
};

// Core parser combinators
pub fn nix_expr(input: &str) -> IResult<&str, NixExpr> {
    ws(alt((
        nix_let_in,
        nix_with,
        nix_if,
        nix_assert,
        nix_lambda,
        nix_binary_expr,
    )))(input)
}

fn nix_binary_expr(input: &str) -> IResult<&str, NixExpr> {
    let (input, left) = nix_unary_expr(input)?;
    let (input, ops) = many0(pair(ws(binary_operator), nix_unary_expr))(input)?;
    
    Ok((input, ops.into_iter().fold(left, |acc, (op, right)| {
        NixExpr::BinaryOp {
            left: Box::new(acc),
            op,
            right: Box::new(right),
        }
    })))
}

fn nix_unary_expr(input: &str) -> IResult<&str, NixExpr> {
    alt((
        map(pair(unary_operator, nix_postfix_expr), |(op, expr)| {
            NixExpr::UnaryOp {
                op,
                expr: Box::new(expr),
            }
        }),
        nix_postfix_expr,
    ))(input)
}

fn nix_postfix_expr(input: &str) -> IResult<&str, NixExpr> {
    let (input, base) = nix_primary_expr(input)?;
    let (input, ops) = many0(alt((
        map(preceded(ws(char('.')), attr_path), PostfixOp::Select),
        map(preceded(ws(tag(" or ")), nix_primary_expr), PostfixOp::SelectDefault),
        map(preceded(ws(char('?')), attr_path), PostfixOp::HasAttr),
        // Fix: Use skip_whitespace_and_comments for function call arguments to handle multi-line whitespace  
        map(preceded(skip_whitespace_and_comments, nix_primary_expr), PostfixOp::FunctionCall),
    )))(input)?;
    
    Ok((input, ops.into_iter().fold(base, |acc, op| match op {
        PostfixOp::FunctionCall(arg) => NixExpr::FunctionCall {
            function: Box::new(acc),
            argument: Box::new(arg),
        },
        PostfixOp::Select(path) => NixExpr::Select {
            expr: Box::new(acc),
            path,
            default: None,
        },
        PostfixOp::SelectDefault(default) => {
            if let NixExpr::Select { expr, path, .. } = acc {
                NixExpr::Select {
                    expr,
                    path,
                    default: Some(Box::new(default)),
                }
            } else {
                acc
            }
        },
        PostfixOp::HasAttr(path) => NixExpr::HasAttr {
            expr: Box::new(acc),
            path,
        },
    })))
}

#[derive(Debug)]
enum PostfixOp {
    FunctionCall(NixExpr),
    Select(AttrPath),
    SelectDefault(NixExpr),
    HasAttr(AttrPath),
}

fn nix_primary_expr(input: &str) -> IResult<&str, NixExpr> {
    ws(alt((
        nix_attrset,
        nix_list,
        nix_interpolated_string,
        nix_literal,
        nix_identifier,
        delimited(char('('), nix_expr, char(')')),
    )))(input)
}

fn nix_literal(input: &str) -> IResult<&str, NixExpr> {
    alt((
        nix_string,
        nix_path,
        nix_uri,
        nix_number,
        nix_bool,
        nix_null,
    ))(input)
}

fn nix_string(input: &str) -> IResult<&str, NixExpr> {
    alt((
        delimited(
            char('"'),
            map(take_until("\""), |s: &str| NixExpr::String(s.to_string())),
            char('"'),
        ),
        delimited(
            tag("''"),
            map(take_until("''"), |s: &str| NixExpr::String(s.to_string())),
            tag("''"),
        ),
    ))(input)
}

fn nix_interpolated_string(input: &str) -> IResult<&str, NixExpr> {
    delimited(
        char('"'),
        map(many0(string_part), |parts| {
            if parts.len() == 1 && matches!(&parts[0], StringPart::Literal(_)) {
                if let StringPart::Literal(s) = &parts[0] {
                    NixExpr::String(s.clone())
                } else {
                    NixExpr::String(String::new())
                }
            } else if parts.is_empty() {
                NixExpr::String(String::new())
            } else if parts.iter().any(|p| matches!(p, StringPart::Interpolation(_))) {
                NixExpr::InterpolatedString(parts)
            } else {
                // All literal parts, concatenate them
                let s = parts.into_iter()
                    .filter_map(|p| match p {
                        StringPart::Literal(s) => Some(s),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("");
                NixExpr::String(s)
            }
        }),
        char('"'),
    )(input)
}

fn string_part(input: &str) -> IResult<&str, StringPart> {
    alt((
        map(
            delimited(tag("${"), nix_expr, char('}')),
            |expr| StringPart::Interpolation(Box::new(expr)),
        ),
        map(
            take_while1(|c| c != '"' && c != '$'),
            |s: &str| StringPart::Literal(s.to_string()),
        ),
        map(
            tag("$"),
            |s: &str| StringPart::Literal(s.to_string()),
        ),
    ))(input)
}

fn nix_path(input: &str) -> IResult<&str, NixExpr> {
    map(
        recognize(pair(
            alt((tag("./"), tag("../"), tag("/"))),
            take_while(|c: char| c.is_alphanumeric() || "/-_.".contains(c)),
        )),
        |s: &str| NixExpr::Path(s.to_string()),
    )(input)
}

fn nix_uri(input: &str) -> IResult<&str, NixExpr> {
    map(
        recognize(tuple((
            alpha1,
            char(':'),
            take_while1(|c: char| c != ' ' && c != '\t' && c != '\n' && c != '\r'),
        ))),
        |s: &str| NixExpr::Uri(s.to_string()),
    )(input)
}

fn nix_number(input: &str) -> IResult<&str, NixExpr> {
    alt((
        map(
            recognize(tuple((digit1, char('.'), digit1))),
            |s: &str| NixExpr::Float(s.parse().unwrap()),
        ),
        map(digit1, |s: &str| NixExpr::Integer(s.parse().unwrap())),
    ))(input)
}

fn nix_bool(input: &str) -> IResult<&str, NixExpr> {
    alt((
        value(NixExpr::Bool(true), tag("true")),
        value(NixExpr::Bool(false), tag("false")),
    ))(input)
}

fn nix_null(input: &str) -> IResult<&str, NixExpr> {
    value(NixExpr::Null, tag("null"))(input)
}

fn nix_identifier(input: &str) -> IResult<&str, NixExpr> {
    map(
        recognize(pair(
            alt((alpha1, tag("_"))),
            take_while(|c: char| c.is_alphanumeric() || c == '_' || c == '-'),
        )),
        |s: &str| NixExpr::Identifier(s.to_string()),
    )(input)
}

fn identifier_string(input: &str) -> IResult<&str, String> {
    map(
        recognize(pair(
            alt((alpha1, tag("_"))),
            take_while(|c: char| c.is_alphanumeric() || c == '_' || c == '-'),
        )),
        |s: &str| s.to_string(),
    )(input)
}

fn nix_attrset(input: &str) -> IResult<&str, NixExpr> {
    let (input, recursive) = opt(ws(tag("rec")))(input)?;
    let (input, _) = ws(char('{'))(input)?;
    let (input, bindings) = separated_list0(ws(char(';')), binding)(input)?;
    let (input, _) = opt(ws(char(';')))(input)?; // Optional trailing semicolon
    let (input, _) = ws(char('}'))(input)?;
    
    Ok((input, NixExpr::AttrSet {
        recursive: recursive.is_some(),
        bindings,
    }))
}

fn nix_list(input: &str) -> IResult<&str, NixExpr> {
    let (input, _) = ws(char('['))(input)?;
    let (input, items) = many0(terminated(ws(nix_list_item), skip_whitespace_and_comments))(input)?;
    let (input, _) = ws(char(']'))(input)?;
    Ok((input, NixExpr::List(items)))
}

fn nix_list_item(input: &str) -> IResult<&str, NixExpr> {
    ws(alt((
        nix_attrset,
        nix_list,
        nix_interpolated_string,
        nix_literal,
        // Handle attribute access like "self.overlays.default" but not function calls
        nix_select_expr,
        nix_identifier,
        delimited(char('('), nix_expr, char(')')),
    )))(input)
}

// Parse identifier with optional select operations (no function calls)
fn nix_select_expr(input: &str) -> IResult<&str, NixExpr> {
    let (input, base) = nix_identifier(input)?;
    let (input, selects) = many0(preceded(ws(char('.')), attr_path_part))(input)?;
    
    Ok((input, selects.into_iter().fold(base, |acc, part| {
        let path = AttrPath { parts: vec![part] };
        NixExpr::Select {
            expr: Box::new(acc),
            path,
            default: None,
        }
    })))
}

fn nix_let_in(input: &str) -> IResult<&str, NixExpr> {
    let (input, _) = ws(tag("let"))(input)?;
    let (input, bindings) = many1(terminated(binding, ws(char(';'))))(input)?;
    let (input, _) = ws(tag("in"))(input)?;
    let (input, body) = nix_expr(input)?;
    
    Ok((input, NixExpr::LetIn {
        bindings,
        body: Box::new(body),
    }))
}

fn nix_with(input: &str) -> IResult<&str, NixExpr> {
    let (input, _) = ws(tag("with"))(input)?;
    let (input, env) = nix_expr(input)?;
    let (input, _) = ws(char(';'))(input)?;
    let (input, body) = nix_expr(input)?;
    
    Ok((input, NixExpr::With {
        env: Box::new(env),
        body: Box::new(body),
    }))
}

fn nix_if(input: &str) -> IResult<&str, NixExpr> {
    let (input, _) = ws(tag("if"))(input)?;
    let (input, condition) = nix_expr(input)?;
    let (input, _) = ws(tag("then"))(input)?;
    let (input, then_expr) = nix_expr(input)?;
    let (input, _) = ws(tag("else"))(input)?;
    let (input, else_expr) = nix_expr(input)?;
    
    Ok((input, NixExpr::If {
        condition: Box::new(condition),
        then_expr: Box::new(then_expr),
        else_expr: Box::new(else_expr),
    }))
}

fn nix_assert(input: &str) -> IResult<&str, NixExpr> {
    let (input, _) = ws(tag("assert"))(input)?;
    let (input, condition) = nix_expr(input)?;
    let (input, _) = ws(char(';'))(input)?;
    let (input, body) = nix_expr(input)?;
    
    Ok((input, NixExpr::Assert {
        condition: Box::new(condition),
        body: Box::new(body),
    }))
}

fn nix_lambda(input: &str) -> IResult<&str, NixExpr> {
    let (input, param) = lambda_param(input)?;
    let (input, _) = ws(char(':'))(input)?;
    let (input, body) = nix_expr(input)?;
    
    Ok((input, NixExpr::Lambda {
        param,
        body: Box::new(body),
    }))
}

fn lambda_param(input: &str) -> IResult<&str, LambdaParam> {
    alt((
        map(
            delimited(
                ws(char('{')),
                pair(
                    separated_list0(ws(char(',')), pattern_param),
                    alt((
                        preceded(ws(char(',')), tag("...")),
                        // Handle trailing comma without ellipsis
                        map(opt(ws(char(','))), |_| ""),
                    )),
                ),
                ws(char('}')),
            ),
            |(params, ellipsis)| LambdaParam::Pattern {
                params,
                ellipsis: ellipsis == "...",
            },
        ),
        map(
            recognize(pair(
                alt((alpha1, tag("_"))),
                take_while(|c: char| c.is_alphanumeric() || c == '_' || c == '-'),
            )),
            |s: &str| LambdaParam::Identifier(s.to_string()),
        ),
    ))(input)
}

fn pattern_param(input: &str) -> IResult<&str, PatternParam> {
    let (input, name) = ws(recognize(pair(
        alt((alpha1, tag("_"))),
        take_while(|c: char| c.is_alphanumeric() || c == '_' || c == '-'),
    )))(input)?;
    let (input, default) = opt(preceded(ws(char('?')), nix_expr))(input)?;
    
    Ok((input, PatternParam {
        name: name.to_string(),
        default: default.map(Box::new),
    }))
}

pub fn binding(input: &str) -> IResult<&str, Binding> {
    alt((
        map(
            tuple((
                ws(tag("inherit")),
                opt(delimited(ws(char('(')), nix_expr, ws(char(')')))),
                // Fix: Parse identifiers separated by whitespace
                separated_list1(multispace1, ws(identifier_string)),
            )),
            |(_, from, attrs)| Binding {
                path: AttrPath { parts: vec![AttrPathPart::Identifier("inherit".to_string())] },
                value: NixExpr::Inherit {
                    from: from.map(Box::new),
                    attrs,
                },
            },
        ),
        map(
            separated_pair(attr_path, ws(char('=')), nix_expr),
            |(path, value)| Binding { path, value },
        ),
    ))(input)
}

fn attr_path(input: &str) -> IResult<&str, AttrPath> {
    map(
        separated_list1(ws(char('.')), attr_path_part),
        |parts| AttrPath { parts },
    )(input)
}

fn attr_path_part(input: &str) -> IResult<&str, AttrPathPart> {
    alt((
        // Handle interpolated strings like "go_1_${toString goVersion}"
        map(nix_interpolated_string, |expr| match expr {
            NixExpr::String(s) => AttrPathPart::String(s),
            NixExpr::InterpolatedString(parts) => {
                // Convert to a single interpolation for simplicity
                if parts.len() == 1 {
                    match &parts[0] {
                        StringPart::Literal(s) => AttrPathPart::String(s.clone()),
                        StringPart::Interpolation(expr) => AttrPathPart::Interpolation(expr.clone()),
                    }
                } else {
                    // For complex interpolated strings, reconstruct the string
                    let reconstructed = parts.iter()
                        .map(|part| match part {
                            StringPart::Literal(s) => s.clone(),
                            StringPart::Interpolation(expr) => format!("${{{}}}", expr.to_nix_string()),
                        })
                        .collect::<String>();
                    AttrPathPart::String(reconstructed)
                }
            }
            _ => AttrPathPart::String("unknown".to_string()),
        }),
        map(
            delimited(tag("${"), nix_expr, char('}')),
            |expr| AttrPathPart::Interpolation(Box::new(expr)),
        ),
        map(
            recognize(pair(
                alt((alpha1, tag("_"))),
                take_while(|c: char| c.is_alphanumeric() || c == '_' || c == '-'),
            )),
            |s: &str| AttrPathPart::Identifier(s.to_string()),
        ),
    ))(input)
}

fn binary_operator(input: &str) -> IResult<&str, BinaryOperator> {
    alt((
        value(BinaryOperator::Eq, tag("==")),
        value(BinaryOperator::Ne, tag("!=")),
        value(BinaryOperator::Le, tag("<=")),
        value(BinaryOperator::Ge, tag(">=")),
        value(BinaryOperator::Lt, char('<')),
        value(BinaryOperator::Gt, char('>')),
        value(BinaryOperator::And, tag("&&")),
        value(BinaryOperator::Or, tag("||")),
        value(BinaryOperator::Implication, tag("->")),
        value(BinaryOperator::Update, tag("//")),
        value(BinaryOperator::Concat, tag("++")),
        value(BinaryOperator::Add, char('+')),
        value(BinaryOperator::Sub, char('-')),
        value(BinaryOperator::Mul, char('*')),
        value(BinaryOperator::Div, char('/')),
    ))(input)
}

fn unary_operator(input: &str) -> IResult<&str, UnaryOperator> {
    alt((
        value(UnaryOperator::Not, char('!')),
        value(UnaryOperator::Negate, char('-')),
    ))(input)
}

fn ws<'a, F, O>(inner: F) -> impl FnMut(&'a str) -> IResult<&'a str, O>
where
    F: FnMut(&'a str) -> IResult<&'a str, O>,
{
    delimited(skip_whitespace_and_comments, inner, skip_whitespace_and_comments)
}

fn skip_whitespace_and_comments(input: &str) -> IResult<&str, ()> {
    let (input, _) = many0(alt((
        map(multispace1, |_| ()),
        map(preceded(char('#'), take_until("\n")), |_| ()),
        map(preceded(char('#'), take_while(|_| true)), |_| ()), // Handle comment at end of file
    )))(input)?;
    Ok((input, ()))
}