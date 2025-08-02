use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Invalid syntax: {0}")]
    InvalidSyntax(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NixExpr {
    // Literals
    String(String),
    Path(String),
    Uri(String),
    Integer(i64),
    Float(f64),
    Bool(bool),
    Null,
    
    // Identifiers
    Identifier(String),
    
    // Collections
    AttrSet {
        recursive: bool,
        bindings: Vec<Binding>,
    },
    List(Vec<NixExpr>),
    
    // String interpolation
    InterpolatedString(Vec<StringPart>),
    
    // Functions
    Lambda {
        param: LambdaParam,
        body: Box<NixExpr>,
    },
    FunctionCall {
        function: Box<NixExpr>,
        argument: Box<NixExpr>,
    },
    
    // Control structures
    LetIn {
        bindings: Vec<Binding>,
        body: Box<NixExpr>,
    },
    With {
        env: Box<NixExpr>,
        body: Box<NixExpr>,
    },
    If {
        condition: Box<NixExpr>,
        then_expr: Box<NixExpr>,
        else_expr: Box<NixExpr>,
    },
    Assert {
        condition: Box<NixExpr>,
        body: Box<NixExpr>,
    },
    
    // Operators
    BinaryOp {
        left: Box<NixExpr>,
        op: BinaryOperator,
        right: Box<NixExpr>,
    },
    UnaryOp {
        op: UnaryOperator,
        expr: Box<NixExpr>,
    },
    
    // Attribute access
    Select {
        expr: Box<NixExpr>,
        path: AttrPath,
        default: Option<Box<NixExpr>>,
    },
    
    // Has attribute
    HasAttr {
        expr: Box<NixExpr>,
        path: AttrPath,
    },
    
    // Inherit expressions
    Inherit {
        from: Option<Box<NixExpr>>,
        attrs: Vec<String>,
    },
}

impl NixExpr {
    pub fn to_nix_string(&self) -> String {
        match self {
            NixExpr::String(s) => format!("\"{}\"", s.replace("\"", "\\\"")),
            NixExpr::Path(p) => p.clone(),
            NixExpr::Uri(u) => u.clone(),
            NixExpr::Integer(i) => i.to_string(),
            NixExpr::Float(f) => f.to_string(),
            NixExpr::Bool(b) => b.to_string(),
            NixExpr::Null => "null".to_string(),
            NixExpr::Identifier(name) => name.clone(),
            NixExpr::AttrSet { recursive, bindings } => {
                let mut result = if *recursive { "rec {\n" } else { "{\n" }.to_string();
                for binding in bindings {
                    // Handle inherit statements specially
                    if let NixExpr::Inherit { from, attrs } = &binding.value {
                        let attr_list = attrs.join(" ");
                        if let Some(from_expr) = from {
                            result.push_str(&format!("  inherit ({}) {};\n", from_expr.to_nix_string(), attr_list));
                        } else {
                            result.push_str(&format!("  inherit {attr_list};\n"));
                        }
                    } else {
                        let path_str = binding.path.parts.iter()
                            .map(|part| match part {
                                AttrPathPart::Identifier(id) => id.clone(),
                                AttrPathPart::String(s) => format!("\"{s}\""),
                                AttrPathPart::Interpolation(expr) => format!("${{{}}}", expr.to_nix_string()),
                            })
                            .collect::<Vec<_>>()
                            .join(".");
                        result.push_str(&format!("  {} = {};\n", path_str, binding.value.to_nix_string()));
                    }
                }
                result.push('}');
                result
            }
            NixExpr::List(items) => {
                let items_str = items.iter()
                    .map(|item| item.to_nix_string())
                    .collect::<Vec<_>>()
                    .join(" ");
                format!("[ {items_str} ]")
            }
            NixExpr::Lambda { param, body } => {
                let param_str = match param {
                    LambdaParam::Identifier(name) => name.clone(),
                    LambdaParam::Pattern { params, ellipsis } => {
                        let param_list = params.iter()
                            .map(|p| if let Some(ref default) = p.default {
                                format!("{} ? {}", p.name, default.to_nix_string())
                            } else {
                                p.name.clone()
                            })
                            .collect::<Vec<_>>()
                            .join(", ");
                        if *ellipsis {
                            format!("{{ {param_list}, ... }}")
                        } else {
                            format!("{{ {param_list} }}")
                        }
                    }
                };
                format!("{}: {}", param_str, body.to_nix_string())
            }
            NixExpr::FunctionCall { function, argument } => {
                format!("{} {}", function.to_nix_string(), argument.to_nix_string())
            }
            NixExpr::Select { expr, path, default } => {
                let path_str = path.parts.iter()
                    .map(|part| match part {
                        AttrPathPart::Identifier(id) => id.clone(),
                        AttrPathPart::String(s) => format!("\"{s}\""),
                        AttrPathPart::Interpolation(expr) => format!("${{{}}}", expr.to_nix_string()),
                    })
                    .collect::<Vec<_>>()
                    .join(".");
                let base = format!("{}.{}", expr.to_nix_string(), path_str);
                if let Some(def) = default {
                    format!("{} or {}", base, def.to_nix_string())
                } else {
                    base
                }
            }
            NixExpr::BinaryOp { left, op, right } => {
                let op_str = match op {
                    BinaryOperator::Concat => "++",
                    BinaryOperator::Add => "+",
                    BinaryOperator::Sub => "-",
                    BinaryOperator::Mul => "*",
                    BinaryOperator::Div => "/",
                    BinaryOperator::Eq => "==",
                    BinaryOperator::Ne => "!=",
                    BinaryOperator::Lt => "<",
                    BinaryOperator::Le => "<=",
                    BinaryOperator::Gt => ">",
                    BinaryOperator::Ge => ">=",
                    BinaryOperator::And => "&&",
                    BinaryOperator::Or => "||",
                    BinaryOperator::Implication => "->",
                    BinaryOperator::Update => "//",
                };
                format!("{} {} {}", left.to_nix_string(), op_str, right.to_nix_string())
            }
            NixExpr::If { condition, then_expr, else_expr } => {
                format!("if {} then {} else {}", 
                    condition.to_nix_string(), 
                    then_expr.to_nix_string(), 
                    else_expr.to_nix_string())
            }
            NixExpr::InterpolatedString(parts) => {
                let content = parts.iter()
                    .map(|part| match part {
                        StringPart::Literal(s) => s.clone(),
                        StringPart::Interpolation(expr) => format!("${{{}}}", expr.to_nix_string()),
                    })
                    .collect::<String>();
                format!("\"{content}\"")
            }
            NixExpr::LetIn { bindings, body } => {
                let mut result = "let\n".to_string();
                for binding in bindings {
                    let path_str = binding.path.parts.iter()
                        .map(|part| match part {
                            AttrPathPart::Identifier(id) => id.clone(),
                            AttrPathPart::String(s) => format!("\"{s}\""),
                            AttrPathPart::Interpolation(expr) => format!("${{{}}}", expr.to_nix_string()),
                        })
                        .collect::<Vec<_>>()
                        .join(".");
                    result.push_str(&format!("  {} = {};\n", path_str, binding.value.to_nix_string()));
                }
                result.push_str(&format!("in\n{}", body.to_nix_string()));
                result
            }
            NixExpr::With { env, body } => {
                format!("with {};\n{}", env.to_nix_string(), body.to_nix_string())
            }
            NixExpr::Inherit { from, attrs } => {
                let attr_list = attrs.join(" ");
                if let Some(from_expr) = from {
                    format!("inherit ({}) {}", from_expr.to_nix_string(), attr_list)
                } else {
                    format!("inherit {attr_list}")
                }
            }
            // Add other cases as needed - for now, fall back to debug for unhandled cases
            _ => format!("(* unhandled: {self:?} *)"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StringPart {
    Literal(String),
    Interpolation(Box<NixExpr>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LambdaParam {
    Identifier(String),
    Pattern {
        params: Vec<PatternParam>,
        ellipsis: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PatternParam {
    pub name: String,
    pub default: Option<Box<NixExpr>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Binding {
    pub path: AttrPath,
    pub value: NixExpr,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttrPath {
    pub parts: Vec<AttrPathPart>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AttrPathPart {
    Identifier(String),
    String(String),
    Interpolation(Box<NixExpr>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BinaryOperator {
    // Arithmetic
    Add, Sub, Mul, Div,
    // Comparison
    Eq, Ne, Lt, Le, Gt, Ge,
    // Logical
    And, Or, Implication,
    // List/String
    Concat, Update,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum UnaryOperator {
    Not,
    Negate,
}

// Flake-specific data structures
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlakeData {
    pub description: Option<String>,
    pub inputs: HashMap<String, NixExpr>,
    pub outputs: Option<NixExpr>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlakeFragments {
    pub header: String,
    pub inputs: HashMap<String, String>,
    pub overlays: HashMap<String, Vec<Binding>>, // Store AST bindings instead of strings
    pub packages: Vec<String>,
    pub env_vars: HashMap<String, String>,
    pub shell_hooks: Vec<String>,
    pub allow_unfree: bool,
    pub let_bindings: HashMap<String, String>,
}