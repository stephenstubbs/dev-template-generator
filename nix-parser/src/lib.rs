use nom::{
    branch::alt,
    bytes::complete::{tag, take_until, take_while, take_while1},
    character::complete::{alpha1, char, digit1, multispace1},
    combinator::{map, opt, recognize, value},
    multi::{many0, many1, separated_list0, separated_list1},
    sequence::{delimited, pair, preceded, separated_pair, terminated, tuple},
    IResult,
};
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

// Main parsing functions
pub fn parse_nix_expr(input: &str) -> Result<NixExpr, ParseError> {
    match ws(nix_expr)(input.trim()) {
        Ok((remaining, expr)) => {
            let remaining_trimmed = remaining.trim();
            if remaining_trimmed.is_empty() {
                Ok(expr)
            } else {
                Err(ParseError::Parse(format!("Unexpected remaining input: '{}' (first 100 chars)", 
                    &remaining_trimmed[..remaining_trimmed.len().min(100)])))
            }
        }
        Err(e) => Err(ParseError::Parse(format!("Parsing Error: {e}"))),
    }
}

pub fn parse_flake(input: &str) -> Result<FlakeData, ParseError> {
    let expr = parse_nix_expr(input)?;
    extract_flake_data(&expr)
}

pub fn extract_flake_fragments(input: &str) -> Result<FlakeFragments, ParseError> {
    let expr = parse_nix_expr(input)?;
    extract_fragments_from_expr(&expr)
}

fn extract_flake_data(expr: &NixExpr) -> Result<FlakeData, ParseError> {
    match expr {
        NixExpr::AttrSet { bindings, .. } => {
            let mut flake = FlakeData {
                description: None,
                inputs: HashMap::new(),
                outputs: None,
            };
            
            for binding in bindings {
                if let [AttrPathPart::Identifier(name)] = &binding.path.parts[..] { match name.as_str() {
                    "description" => {
                        if let NixExpr::String(desc) = &binding.value {
                            flake.description = Some(desc.clone());
                        }
                    }
                    "inputs" => {
                        if let NixExpr::AttrSet { bindings, .. } = &binding.value {
                            for input_binding in bindings {
                                if let [AttrPathPart::Identifier(input_name)] = &input_binding.path.parts[..] {
                                    flake.inputs.insert(input_name.clone(), input_binding.value.clone());
                                }
                            }
                        }
                    }
                    "outputs" => {
                        flake.outputs = Some(binding.value.clone());
                    }
                    _ => {}
                } }
            }
            
            Ok(flake)
        }
        _ => Err(ParseError::InvalidSyntax("Expected attribute set for flake".to_string())),
    }
}

fn extract_fragments_from_expr(expr: &NixExpr) -> Result<FlakeFragments, ParseError> {
    let mut fragments = FlakeFragments {
        header: String::new(),
        inputs: HashMap::new(),
        overlays: HashMap::new(),
        packages: Vec::new(),
        env_vars: HashMap::new(),
        shell_hooks: Vec::new(),
        allow_unfree: false,
        let_bindings: HashMap::new(),
    };
    
    if let NixExpr::AttrSet { bindings, .. } = expr {
        for binding in bindings {
            match &binding.path.parts[..] {
                [AttrPathPart::Identifier(name)] => match name.as_str() {
                    "description" => {
                        if let NixExpr::String(desc) = &binding.value {
                            fragments.header = desc.clone();
                        }
                    }
                    "inputs" => {
                        extract_inputs_from_expr(&binding.value, &mut fragments.inputs);
                    }
                    "outputs" => {
                        extract_outputs_from_expr(&binding.value, &mut fragments);
                    }
                    _ => {}
                },
                // Handle multi-part paths like "inputs.nixpkgs.url"
                [AttrPathPart::Identifier(first), AttrPathPart::Identifier(second), AttrPathPart::Identifier(third)] => {
                    if first == "inputs" && third == "url" {
                        if let NixExpr::String(url) = &binding.value {
                            fragments.inputs.insert(second.clone(), url.clone());
                        }
                    }
                },
                _ => {}
            }
        }
    }
    
    Ok(fragments)
}

fn extract_inputs_from_expr(expr: &NixExpr, inputs: &mut HashMap<String, String>) {
    if let NixExpr::AttrSet { bindings, .. } = expr {
        for binding in bindings {
            if let [AttrPathPart::Identifier(input_name)] = &binding.path.parts[..] {
                match &binding.value {
                    // Simple format: nixpkgs.url = "...";
                    NixExpr::AttrSet { bindings, .. } => {
                        for url_binding in bindings {
                            if let [AttrPathPart::Identifier(attr)] = &url_binding.path.parts[..] {
                                if attr == "url" {
                                    if let NixExpr::String(url) = &url_binding.value {
                                        inputs.insert(input_name.clone(), url.clone());
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            } else if binding.path.parts.len() == 2 {
                // Handle nixpkgs.url format
                if let [AttrPathPart::Identifier(input_name), AttrPathPart::Identifier(attr)] = &binding.path.parts[..] {
                    if attr == "url" {
                        if let NixExpr::String(url) = &binding.value {
                            inputs.insert(input_name.clone(), url.clone());
                        }
                    }
                }
            }
        }
    }
}

fn extract_outputs_from_expr(expr: &NixExpr, fragments: &mut FlakeFragments) {
    // Outputs is typically a lambda function
    if let NixExpr::Lambda { body, .. } = expr {
        extract_outputs_body(body, fragments);
    }
    
    // Also search for config.allowUnfree patterns anywhere in the expression
    if detect_allow_unfree(expr) {
        fragments.allow_unfree = true;
    }
}

fn detect_allow_unfree(expr: &NixExpr) -> bool {
    match expr {
        NixExpr::AttrSet { bindings, .. } => {
            for binding in bindings {
                // Check for config.allowUnfree = true pattern
                if let [AttrPathPart::Identifier(first), AttrPathPart::Identifier(second)] = &binding.path.parts[..] {
                    if first == "config" && second == "allowUnfree" {
                        if let NixExpr::Bool(true) = &binding.value {
                            return true;
                        }
                    }
                }
                // Recursively check the binding value
                if detect_allow_unfree(&binding.value) {
                    return true;
                }
            }
            false
        }
        NixExpr::Lambda { body, .. } => detect_allow_unfree(body),
        NixExpr::LetIn { bindings, body } => {
            for binding in bindings {
                if detect_allow_unfree(&binding.value) {
                    return true;
                }
            }
            detect_allow_unfree(body)
        }
        NixExpr::FunctionCall { function, argument } => {
            detect_allow_unfree(function) || detect_allow_unfree(argument)
        }
        NixExpr::List(items) => {
            items.iter().any(detect_allow_unfree)
        }
        NixExpr::With { env, body } => {
            detect_allow_unfree(env) || detect_allow_unfree(body)
        }
        NixExpr::If { condition, then_expr, else_expr } => {
            detect_allow_unfree(condition) || detect_allow_unfree(then_expr) || detect_allow_unfree(else_expr)
        }
        _ => false,
    }
}

fn extract_outputs_body(expr: &NixExpr, fragments: &mut FlakeFragments) {
    match expr {
        NixExpr::LetIn { bindings, body } => {
            // Extract let bindings first
            extract_let_bindings(bindings, &mut fragments.let_bindings);
            // Then process the body
            extract_outputs_body(body, fragments);
        }
        NixExpr::AttrSet { bindings, .. } => {
            for binding in bindings {
                match &binding.path.parts[..] {
                    [AttrPathPart::Identifier(name)] => match name.as_str() {
                        "inputs" => {
                            extract_inputs_from_expr(&binding.value, &mut fragments.inputs);
                        }
                        "overlays" => {
                            extract_overlays_from_expr(&binding.value, fragments);
                        }
                        "devShells" => {
                            extract_devshells_from_expr(&binding.value, fragments);
                        }
                        _ => {}
                    },
                    // Handle nested paths like "overlays.default" and "inputs.nixpkgs.url"
                    [AttrPathPart::Identifier(first), AttrPathPart::Identifier(second)] => {
                        if first == "overlays" {
                            // Extract the overlay body bindings (inside the lambda)
                            let overlay_bindings = extract_overlay_bindings(&binding.value);
                            fragments.overlays.insert(second.clone(), overlay_bindings);
                        }
                    },
                    // Handle inputs.nixpkgs.url format
                    [AttrPathPart::Identifier(first), AttrPathPart::Identifier(second), AttrPathPart::Identifier(third)] => {
                        if first == "inputs" && third == "url" {
                            if let NixExpr::String(url) = &binding.value {
                                fragments.inputs.insert(second.clone(), url.clone());
                            }
                        }
                    },
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

fn extract_overlays_from_expr(expr: &NixExpr, fragments: &mut FlakeFragments) {
    if let NixExpr::AttrSet { bindings, .. } = expr {
        for binding in bindings {
            if let [AttrPathPart::Identifier(overlay_name)] = &binding.path.parts[..] {
                // Extract the overlay body bindings (inside the lambda)
                let overlay_bindings = extract_overlay_bindings(&binding.value);
                fragments.overlays.insert(overlay_name.clone(), overlay_bindings);
            }
        }
    }
}

fn extract_overlay_bindings(expr: &NixExpr) -> Vec<Binding> {
    match expr {
        // Handle final: prev: { ... } or final: prev: rec { ... }
        NixExpr::Lambda { body, .. } => {
            if let NixExpr::Lambda { body: inner_body, .. } = body.as_ref() {
                // Double lambda: final: prev: { ... }
                extract_overlay_attrset_bindings(inner_body)
            } else {
                // Single lambda, extract its body
                extract_overlay_attrset_bindings(body)
            }
        }
        // Direct attribute set (shouldn't happen for overlays, but handle it)
        _ => extract_overlay_attrset_bindings(expr),
    }
}

fn extract_overlay_attrset_bindings(expr: &NixExpr) -> Vec<Binding> {
    match expr {
        NixExpr::AttrSet { bindings, .. } => bindings.clone(),
        NixExpr::LetIn { bindings, body } => {
            // For let-in expressions in overlays, we need to collect both let bindings and body bindings
            let mut result = bindings.clone();
            result.extend(extract_overlay_attrset_bindings(body));
            result
        }
        _ => Vec::new(),
    }
}


fn extract_let_bindings(bindings: &[Binding], let_bindings: &mut HashMap<String, String>) {
    for binding in bindings {
        if let [AttrPathPart::Identifier(name)] = &binding.path.parts[..] {
            // Only extract simple bindings (literals, simple expressions)
            if is_simple_binding(&binding.value) {
                let value = binding.value.to_nix_string();
                let_bindings.insert(name.clone(), value);
            }
        }
    }
}

fn is_simple_binding(expr: &NixExpr) -> bool {
    match expr {
        // Simple literals
        NixExpr::Integer(_) | NixExpr::Float(_) | NixExpr::Bool(_) | NixExpr::String(_) => true,
        // Simple identifiers
        NixExpr::Identifier(_) => true,
        // Simple lists of identifiers/literals
        NixExpr::List(items) => items.iter().all(is_simple_binding),
        // Skip complex expressions like lambdas, function calls, etc.
        _ => false,
    }
}

fn extract_devshells_from_expr(expr: &NixExpr, fragments: &mut FlakeFragments) {
    // Navigate through the devShells structure to find mkShell calls
    find_packages_in_expr(expr, &mut fragments.packages);
    find_env_in_expr(expr, &mut fragments.env_vars);
    find_shell_hooks_in_expr(expr, &mut fragments.shell_hooks);
}

fn find_packages_in_expr(expr: &NixExpr, packages: &mut Vec<String>) {
    match expr {
        NixExpr::AttrSet { bindings, .. } => {
            for binding in bindings {
                if let [AttrPathPart::Identifier(name)] = &binding.path.parts[..] {
                    if name == "packages" {
                        extract_packages_from_value(&binding.value, packages);
                    }
                }
                find_packages_in_expr(&binding.value, packages);
            }
        }
        NixExpr::List(items) => {
            for item in items {
                find_packages_in_expr(item, packages);
            }
        }
        NixExpr::With { body, .. } => {
            find_packages_in_expr(body, packages);
        }
        NixExpr::FunctionCall { argument, .. } => {
            find_packages_in_expr(argument, packages);
        }
        NixExpr::LetIn { body, .. } => {
            find_packages_in_expr(body, packages);
        }
        NixExpr::Lambda { body, .. } => {
            find_packages_in_expr(body, packages);
        }
        _ => {}
    }
}

fn extract_packages_from_value(expr: &NixExpr, packages: &mut Vec<String>) {
    match expr {
        NixExpr::With { body, .. } => {
            // Recursively extract from the body of the with expression
            extract_packages_from_value(body, packages);
        }
        NixExpr::List(items) => {
            for item in items {
                if let NixExpr::Identifier(name) = item {
                    packages.push(name.clone());
                }
            }
        }
        NixExpr::BinaryOp { left, op: BinaryOperator::Concat, right } => {
            // Handle concatenation operations recursively
            extract_packages_from_value(left, packages);
            extract_packages_from_value(right, packages);
        }
        NixExpr::If { then_expr, else_expr, .. } => {
            // Handle conditional expressions - extract from both branches
            extract_packages_from_value(then_expr, packages);
            extract_packages_from_value(else_expr, packages);
        }
        _ => {}
    }
}


fn find_env_in_expr(expr: &NixExpr, env_vars: &mut HashMap<String, String>) {
    if let NixExpr::AttrSet { bindings, .. } = expr {
        for binding in bindings {
            if let [AttrPathPart::Identifier(name)] = &binding.path.parts[..] {
                if name == "env" {
                    if let NixExpr::AttrSet { bindings, .. } = &binding.value {
                        for env_binding in bindings {
                            if let [AttrPathPart::Identifier(env_name)] = &env_binding.path.parts[..] {
                                if let NixExpr::String(env_value) = &env_binding.value {
                                    env_vars.insert(env_name.clone(), env_value.clone());
                                }
                            }
                        }
                    }
                }
            }
            find_env_in_expr(&binding.value, env_vars);
        }
    }
}

fn find_shell_hooks_in_expr(expr: &NixExpr, shell_hooks: &mut Vec<String>) {
    match expr {
        NixExpr::AttrSet { bindings, .. } => {
            for binding in bindings {
                if let [AttrPathPart::Identifier(name)] = &binding.path.parts[..] {
                    if name == "shellHook" {
                        if let NixExpr::String(hook) = &binding.value {
                            shell_hooks.push(hook.clone());
                        }
                    } else if name.contains("venvShellHook") {
                        shell_hooks.push("python-venv".to_string());
                    }
                }
                find_shell_hooks_in_expr(&binding.value, shell_hooks);
            }
        }
        NixExpr::Lambda { body, .. } => {
            find_shell_hooks_in_expr(body, shell_hooks);
        }
        NixExpr::FunctionCall { argument, .. } => {
            find_shell_hooks_in_expr(argument, shell_hooks);
        }
        NixExpr::LetIn { body, .. } => {
            find_shell_hooks_in_expr(body, shell_hooks);
        }
        NixExpr::With { body, .. } => {
            find_shell_hooks_in_expr(body, shell_hooks);
        }
        _ => {}
    }
}

// Core parser combinators
fn nix_expr(input: &str) -> IResult<&str, NixExpr> {
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

fn binding(input: &str) -> IResult<&str, Binding> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_attrset() {
        let input = r#"{ foo = "bar"; }"#;
        let result = parse_nix_expr(input).unwrap();
        
        match result {
            NixExpr::AttrSet { bindings, .. } => {
                assert_eq!(bindings.len(), 1);
                assert_eq!(bindings[0].path.parts[0], AttrPathPart::Identifier("foo".to_string()));
                assert_eq!(bindings[0].value, NixExpr::String("bar".to_string()));
            }
            _ => panic!("Expected AttrSet"),
        }
    }

    #[test]
    fn test_parse_flake_description() {
        let input = r#"{ description = "A test flake"; }"#;
        let flake = parse_flake(input).unwrap();
        assert_eq!(flake.description, Some("A test flake".to_string()));
    }

    #[test]
    fn test_parse_function_call() {
        let input = r#"pkgs.mkShell { buildInputs = [ go ]; }"#;
        let result = parse_nix_expr(input).unwrap();
        
        match result {
            NixExpr::FunctionCall { function, argument } => {
                match *function {
                    NixExpr::Select { .. } => {},
                    _ => panic!("Expected Select expression"),
                }
                match *argument {
                    NixExpr::AttrSet { .. } => {},
                    _ => panic!("Expected AttrSet argument"),
                }
            }
            _ => panic!("Expected FunctionCall"),
        }
    }

    #[test]
    fn test_parse_lambda() {
        let input = r#"{ pkgs }: pkgs.hello"#;
        let result = parse_nix_expr(input).unwrap();
        
        match result {
            NixExpr::Lambda { param, body } => {
                match param {
                    LambdaParam::Pattern { params, .. } => {
                        assert_eq!(params[0].name, "pkgs");
                    }
                    _ => panic!("Expected pattern parameter"),
                }
                match *body {
                    NixExpr::Select { .. } => {},
                    _ => panic!("Expected select expression in body"),
                }
            }
            _ => panic!("Expected Lambda"),
        }
    }

    #[test]
    fn test_parse_let_in() {
        let input = r#"let x = 1; in x + 2"#;
        let result = parse_nix_expr(input).unwrap();
        
        match result {
            NixExpr::LetIn { bindings, body } => {
                assert_eq!(bindings.len(), 1);
                assert_eq!(bindings[0].path.parts[0], AttrPathPart::Identifier("x".to_string()));
                assert_eq!(bindings[0].value, NixExpr::Integer(1));
                match *body {
                    NixExpr::BinaryOp { .. } => {},
                    _ => panic!("Expected binary operation in body"),
                }
            }
            _ => panic!("Expected LetIn"),
        }
    }

    #[test]
    fn test_parse_list() {
        let input = r#"[ "a" "b" "c" ]"#;
        let result = parse_nix_expr(input).unwrap();
        
        match result {
            NixExpr::List(items) => {
                assert_eq!(items.len(), 3);
                assert_eq!(items[0], NixExpr::String("a".to_string()));
                assert_eq!(items[1], NixExpr::String("b".to_string()));
                assert_eq!(items[2], NixExpr::String("c".to_string()));
            }
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn test_parse_interpolated_string() {
        let input = r#""Hello ${name}!""#;
        let result = parse_nix_expr(input).unwrap();
        
        match result {
            NixExpr::InterpolatedString(parts) => {
                assert_eq!(parts.len(), 3);
                assert_eq!(parts[0], StringPart::Literal("Hello ".to_string()));
                match &parts[1] {
                    StringPart::Interpolation(expr) => {
                        assert_eq!(**expr, NixExpr::Identifier("name".to_string()));
                    }
                    _ => panic!("Expected interpolation"),
                }
                assert_eq!(parts[2], StringPart::Literal("!".to_string()));
            }
            _ => panic!("Expected InterpolatedString"),
        }
    }

    #[test]
    fn test_extract_flake_fragments_rust() {
        let input = include_str!("templates/rust.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert_eq!(result.header, "A Nix-flake-based Rust development environment");
        assert_eq!(result.inputs.len(), 2);
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(result.inputs.contains_key("rust-overlay"));
        assert!(!result.overlays.is_empty());
        assert!(!result.packages.is_empty());
        assert!(result.packages.contains(&"rustToolchain".to_string()));
    }

    #[test]
    fn test_extract_flake_fragments_python() {
        let input = include_str!("templates/python.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        
        assert_eq!(result.header, "A Nix-flake-based Python development environment");
        assert_eq!(result.inputs.len(), 1);
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
        assert!(!result.shell_hooks.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_go() {
        let input = include_str!("templates/go.nix");
        
        // First try to parse the basic expression
        match parse_nix_expr(input) {
            Ok(_expr) => {
                let result = extract_flake_fragments(input).unwrap();
                
                assert_eq!(result.header, "A Nix-flake-based Go 1.22 development environment");
                assert!(!result.overlays.is_empty());
                assert!(!result.packages.is_empty());
            }
            Err(e) => {
                eprintln!("Failed to parse go.nix template: {e:#?}");
                // For now, let's not panic so we can see what's happening
                assert!(false, "Failed to parse go.nix template");
            }
        }
    }

    #[test]
    fn test_extract_flake_fragments_elm() {
        let input = include_str!("templates/elm.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert_eq!(result.header, "A Nix-flake-based Elm development environment");
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_node() {
        let input = include_str!("templates/node.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert_eq!(result.header, "A Nix-flake-based Node.js development environment");
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.overlays.is_empty());
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_java() {
        let input = include_str!("templates/java.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert_eq!(result.header, "A Nix-flake-based Java development environment");
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.overlays.is_empty());
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_haskell() {
        let input = include_str!("templates/haskell.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert_eq!(result.header, "A Nix-flake-based Haskell development environment");
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_c_cpp() {
        let input = include_str!("templates/c-cpp.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert_eq!(result.header, "A Nix-flake-based C/C++ development environment");
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_shell() {
        let input = include_str!("templates/shell.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert_eq!(result.header, "A Nix-flake-based Shell development environment");
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_bun() {
        let input = include_str!("templates/bun.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_clojure() {
        let input = include_str!("templates/clojure.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_csharp() {
        let input = include_str!("templates/csharp.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_cue() {
        let input = include_str!("templates/cue.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_dhall() {
        let input = include_str!("templates/dhall.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_elixir() {
        let input = include_str!("templates/elixir.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_gleam() {
        let input = include_str!("templates/gleam.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_hashi() {
        let input = include_str!("templates/hashi.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
        assert!(result.allow_unfree, "Hashi template should set allow_unfree = true");
    }

    #[test]
    fn test_extract_flake_fragments_haxe() {
        let input = include_str!("templates/haxe.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_kotlin() {
        let input = include_str!("templates/kotlin.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_latex() {
        let input = include_str!("templates/latex.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_nickel() {
        let input = include_str!("templates/nickel.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_nim() {
        let input = include_str!("templates/nim.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_nix() {
        let input = include_str!("templates/nix.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_ocaml() {
        let input = include_str!("templates/ocaml.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_opa() {
        let input = include_str!("templates/opa.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_php() {
        let input = include_str!("templates/php.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_protobuf() {
        let input = include_str!("templates/protobuf.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_pulumi() {
        let input = include_str!("templates/pulumi.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_r() {
        let input = include_str!("templates/r.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_ruby() {
        let input = include_str!("templates/ruby.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_rust_toolchain() {
        let input = include_str!("templates/rust-toolchain.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_scala() {
        let input = include_str!("templates/scala.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_swift() {
        let input = include_str!("templates/swift.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_vlang() {
        let input = include_str!("templates/vlang.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_extract_flake_fragments_zig() {
        let input = include_str!("templates/zig.nix");
        let result = extract_flake_fragments(input).unwrap();
        
        assert!(result.inputs.contains_key("nixpkgs"));
        assert!(!result.packages.is_empty());
    }

    #[test]
    fn test_interpolated_attribute_access() {
        let input = r#"final."go_1_${toString goVersion}""#;
        let result = parse_nix_expr(input);
        
        match result {
            Ok(expr) => {
                eprintln!("Parsed interpolated attribute access: {expr:#?}");
            }
            Err(e) => {
                eprintln!("Failed to parse interpolated attribute access: {e:#?}");
                // For now, let's see what happens
            }
        }
    }

    #[test]  
    fn test_complex_lambda_params() {
        let input = r#"{ self, nixpkgs, rust-overlay }: body"#;
        let result = parse_nix_expr(input);
        
        match result {
            Ok(expr) => {
                eprintln!("Parsed complex lambda: {expr:#?}");
            }
            Err(e) => {
                eprintln!("Failed to parse complex lambda: {e:#?}");
            }
        }
    }

    #[test]
    fn test_multiline_lambda() {
        let input = r#"{
  self,
  nixpkgs,
  rust-overlay,
}: body"#;
        let result = parse_nix_expr(input);
        
        match result {
            Ok(expr) => {
                eprintln!("Parsed multiline lambda: {expr:#?}");
            }
            Err(e) => {
                eprintln!("Failed to parse multiline lambda: {e:#?}");
            }
        }
    }

    #[test]
    fn test_go_template_minimal() {
        let input = r#"{
  description = "A Nix-flake-based Go 1.22 development environment";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }: {
    overlays.default = final: prev: {
      go = final."go_1_24";
    };
  };
}"#;
        let result = parse_nix_expr(input);
        
        match result {
            Ok(expr) => {
                eprintln!("Parsed minimal go template: {expr:#?}");
            }
            Err(e) => {
                eprintln!("Failed to parse minimal go template: {e:#?}");
            }
        }
    }

    #[test]
    fn test_function_call_in_interpolation() {
        let input = r#""go_1_${toString goVersion}""#;
        let result = parse_nix_expr(input);
        
        match result {
            Ok(expr) => {
                eprintln!("Parsed function call in interpolation: {expr:#?}");
            }
            Err(e) => {
                eprintln!("Failed to parse function call in interpolation: {e:#?}");  
            }
        }
    }

    #[test]
    fn test_comment_in_binding() {
        let input = r#"goVersion = 24; # Change this to update the whole stack"#;
        let result = binding(input);
        
        match result {
            Ok((remaining, binding)) => {
                eprintln!("Parsed binding with comment: {binding:#?}, remaining: '{remaining}'");
            }
            Err(e) => {
                eprintln!("Failed to parse binding with comment: {e:#?}");
            }
        }
    }

    #[test]
    fn test_inherit_statement() {
        let input = r#"{ inherit system; }"#;
        let result = parse_nix_expr(input);
        
        match result {
            Ok(expr) => {
                eprintln!("Parsed inherit statement: {expr:#?}");
            }
            Err(e) => {
                eprintln!("Failed to parse inherit statement: {e:#?}");
            }
        }
    }

    #[test]
    fn test_import_function() {
        let input = r#"import nixpkgs { inherit system; }"#;
        let result = parse_nix_expr(input);
        
        match result {
            Ok(expr) => {
                eprintln!("Parsed import function: {expr:#?}");
            }
            Err(e) => {
                eprintln!("Failed to parse import function: {e:#?}");
            }
        }
    }

    #[test]
    fn test_progressive_go_parsing() {
        // Test increasingly complex parts of the go template
        
        // Just the description
        let input1 = r#"{ description = "A Nix-flake-based Go 1.22 development environment"; }"#;
        match parse_nix_expr(input1) {
            Ok(_) => eprintln!("✓ Parsed basic description"),
            Err(e) => eprintln!("✗ Failed to parse basic description: {e:#?}"),
        }
        
        // Test 7: Exact problematic pattern from go template
        let exact_go_part = r#"{
  description = "A Nix-flake-based Go 1.22 development environment";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      goVersion = 24; # Change this to update the whole stack

      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forEachSupportedSystem =
        f:
        nixpkgs.lib.genAttrs supportedSystems (
          system:
          f {
            pkgs = import nixpkgs {
              inherit system;
              overlays = [ self.overlays.default ];
            };
          }
        );
    in
    {
      overlays.default = final: prev: {
        go = final."go_1_${toString goVersion}";
      };

      devShells = forEachSupportedSystem (
        { pkgs }:
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              go
              gotools
              golangci-lint
            ];
          };
        }
      );
    };
}"#;
        match parse_nix_expr(exact_go_part) {
            Ok(_) => eprintln!("✓ Parsed full go template"),
            Err(e) => eprintln!("✗ Failed to parse full go template: {e:#?}"),
        }
        
        // Test 8: Let's try without the complex interpolation
        let without_interpolation = r#"{
  description = "A Nix-flake-based Go 1.22 development environment";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      goVersion = 24; # Change this to update the whole stack

      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forEachSupportedSystem =
        f:
        nixpkgs.lib.genAttrs supportedSystems (
          system:
          f {
            pkgs = import nixpkgs {
              inherit system;
              overlays = [ self.overlays.default ];
            };
          }
        );
    in
    {
      overlays.default = final: prev: {
        go = final.go_1_24;
      };

      devShells = forEachSupportedSystem (
        { pkgs }:
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              go
              gotools  
              golangci-lint
            ];
          };
        }
      );
    };
}"#;
        match parse_nix_expr(without_interpolation) {
            Ok(_) => eprintln!("✓ Parsed without interpolation"),
            Err(e) => eprintln!("✗ Failed without interpolation: {e:#?}"),
        }
        
        // Test 9: Try parsing just the problematic structure step by step
        let just_outputs = r#"{
  outputs =
    { self, nixpkgs }:
    let
      goVersion = 24;
    in
    {
      overlays.default = final: prev: {
        go = final.go_1_24;
      };
    };
}"#;
        match parse_nix_expr(just_outputs) {
            Ok(_) => eprintln!("✓ Parsed just outputs"),
            Err(e) => eprintln!("✗ Failed just outputs: {e:#?}"),
        }
        
        // Test 10: Combine description + inputs + simple outputs
        let desc_inputs_outputs = r#"{
  description = "A Nix-flake-based Go 1.22 development environment";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }: {};
}"#;
        match parse_nix_expr(desc_inputs_outputs) {
            Ok(_) => eprintln!("✓ Parsed desc+inputs+outputs"),
            Err(e) => eprintln!("✗ Failed desc+inputs+outputs: {e:#?}"),
        }
        
        // Test 11: Add the let-in with complex expressions gradually
        let with_complex_let = r#"{
  description = "A Nix-flake-based Go 1.22 development environment";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      goVersion = 24; # Change this to update the whole stack

      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin" 
        "aarch64-darwin"
      ];
    in
    {};
}"#;
        match parse_nix_expr(with_complex_let) {
            Ok(_) => eprintln!("✓ Parsed with complex let"),
            Err(e) => eprintln!("✗ Failed with complex let: {e:#?}"),
        }
        
        // Test 12: Add the complex forEachSupportedSystem lambda
        let with_complex_lambda = r#"{
  description = "A Nix-flake-based Go 1.22 development environment";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      goVersion = 24; # Change this to update the whole stack

      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin" 
        "aarch64-darwin"
      ];
      forEachSupportedSystem =
        f:
        nixpkgs.lib.genAttrs supportedSystems (
          system:
          f {
            pkgs = import nixpkgs {
              inherit system;
              overlays = [ self.overlays.default ];
            };
          }
        );
    in
    {};
}"#;
        match parse_nix_expr(with_complex_lambda) {
            Ok(_) => eprintln!("✓ Parsed with complex lambda"),
            Err(e) => eprintln!("✗ Failed with complex lambda: {e:#?}"),
        }
        
        // Test 13: Test just the problematic lambda in isolation  
        let just_problematic_lambda = r#"forEachSupportedSystem =
        f:
        nixpkgs.lib.genAttrs supportedSystems (
          system:
          f {
            pkgs = import nixpkgs {
              inherit system;
              overlays = [ self.overlays.default ];
            };
          }
        )"#;
        match binding(just_problematic_lambda) {
            Ok((remaining, _binding)) => eprintln!("✓ Parsed problematic lambda binding, remaining: '{remaining}'"),
            Err(e) => eprintln!("✗ Failed problematic lambda binding: {e:#?}"),
        }
        
        // Test 14: Test just the function call part that's failing
        let just_function_call = r#"nixpkgs.lib.genAttrs supportedSystems (
          system:
          f {}
        )"#;
        match nix_expr(just_function_call) {
            Ok((remaining, _expr)) => eprintln!("✓ Parsed function call, remaining: '{remaining}'"),
            Err(e) => eprintln!("✗ Failed function call: {e:#?}"),
        }
        
        // Test 15: Test the exact complex function call that's failing
        let exact_complex_call = r#"nixpkgs.lib.genAttrs supportedSystems (
          system:
          f {
            pkgs = import nixpkgs {
              inherit system;
              overlays = [ self.overlays.default ];
            };
          }
        )"#;
        match nix_expr(exact_complex_call) {
            Ok((remaining, _expr)) => eprintln!("✓ Parsed exact complex call, remaining: '{remaining}'"),
            Err(e) => eprintln!("✗ Failed exact complex call: {e:#?}"),
        }
        
        // Test 16: Test just the parenthesized lambda in isolation
        let just_parenthesized_lambda = r#"(
          system:
          f {
            pkgs = import nixpkgs {
              inherit system;
              overlays = [ self.overlays.default ];
            };
          }
        )"#;
        match nix_expr(just_parenthesized_lambda) {
            Ok((remaining, _expr)) => eprintln!("✓ Parsed parenthesized lambda, remaining: '{remaining}'"),
            Err(e) => eprintln!("✗ Failed parenthesized lambda: {e:#?}"),
        }
        
        // Test 17: Test the exact failing attribute set
        let failing_attrset = r#"{
            pkgs = import nixpkgs {
              inherit system;
              overlays = [ self.overlays.default ];
            };
          }"#;
        match nix_expr(failing_attrset) {
            Ok((remaining, _expr)) => eprintln!("✓ Parsed failing attrset, remaining: '{remaining}'"),
            Err(e) => eprintln!("✗ Failed failing attrset: {e:#?}"),
        }
        
        // Test 18: Test just the inherit statement
        let just_inherit = r#"inherit system"#;
        match binding(just_inherit) {
            Ok((remaining, _binding)) => eprintln!("✓ Parsed inherit, remaining: '{remaining}'"),
            Err(e) => eprintln!("✗ Failed inherit: {e:#?}"),
        }
        
        // Test 19: Test nested attrset with inherit
        let nested_inherit = r#"{
  inherit system;
  foo = "bar";
}"#;
        match nix_expr(nested_inherit) {
            Ok((remaining, _expr)) => eprintln!("✓ Parsed nested inherit, remaining: '{remaining}'"),
            Err(e) => eprintln!("✗ Failed nested inherit: {e:#?}"),
        }
        
        // Test 20: Test import function call
        let import_call = r#"import nixpkgs {
  inherit system;
  overlays = [ self.overlays.default ];
}"#;
        match nix_expr(import_call) {
            Ok((remaining, _expr)) => eprintln!("✓ Parsed import call, remaining: '{remaining}'"),
            Err(e) => eprintln!("✗ Failed import call: {e:#?}"),
        }
        
        // Test 21: Test just the problematic attribute set again
        let just_the_attrset = r#"{
  inherit system;
  overlays = [ self.overlays.default ];
}"#;
        match nix_expr(just_the_attrset) {
            Ok((remaining, _expr)) => eprintln!("✓ Parsed just attrset, remaining: '{remaining}'"),
            Err(e) => eprintln!("✗ Failed just attrset: {e:#?}"),
        }
        
        // Test 22: Test inherit with semicolon
        let inherit_with_semicolon = r#"inherit system;"#;
        match binding(inherit_with_semicolon) {
            Ok((remaining, _binding)) => eprintln!("✓ Parsed inherit with semicolon, remaining: '{remaining}'"),
            Err(e) => eprintln!("✗ Failed inherit with semicolon: {e:#?}"),
        }
        
        // Test 23: Test the exact case that should work now
        let inherit_no_semicolon = r#"inherit system"#;
        match binding(inherit_no_semicolon) {
            Ok((remaining, _binding)) => eprintln!("✓ Parsed inherit no semicolon, remaining: '{remaining}'"),
            Err(e) => eprintln!("✗ Failed inherit no semicolon: {e:#?}"),
        }
        
        // Test 24: Very basic attrset with inherit
        let basic_inherit_attrset = r#"{ inherit system; }"#;
        match nix_expr(basic_inherit_attrset) {
            Ok((remaining, _expr)) => eprintln!("✓ Parsed basic inherit attrset, remaining: '{remaining}'"),
            Err(e) => eprintln!("✗ Failed basic inherit attrset: {e:#?}"),
        }
        
        // Test 25: Just the overlays binding that's failing
        let just_overlays = r#"overlays = [ self.overlays.default ]"#;
        match binding(just_overlays) {
            Ok((remaining, _binding)) => eprintln!("✓ Parsed overlays binding, remaining: '{remaining}'"),
            Err(e) => eprintln!("✗ Failed overlays binding: {e:#?}"),
        }
        
        // Test 26: Just the list that's failing
        let just_list = r#"[ self.overlays.default ]"#;
        match nix_expr(just_list) {
            Ok((remaining, _expr)) => eprintln!("✓ Parsed overlays list, remaining: '{remaining}'"),
            Err(e) => eprintln!("✗ Failed overlays list: {e:#?}"),
        }
        
        // Test 27: Just the attribute access that might be failing
        let just_attr_access = r#"self.overlays.default"#;
        match nix_expr(just_attr_access) {
            Ok((remaining, _expr)) => eprintln!("✓ Parsed attr access, remaining: '{remaining}'"),
            Err(e) => eprintln!("✗ Failed attr access: {e:#?}"),
        }
        
        // Test 28: Test the problematic packages list
        let packages_list = r#"[
              go
              gotools
              golangci-lint
            ]"#;
        match nix_expr(packages_list) {
            Ok((remaining, expr)) => eprintln!("✓ Parsed packages list: {expr:?}, remaining: '{remaining}'"),
            Err(e) => eprintln!("✗ Failed packages list: {e:#?}"),
        }
        
        // Add inputs
        let input2 = r#"{
  description = "A Nix-flake-based Go 1.22 development environment";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
}"#;
        match parse_nix_expr(input2) {
            Ok(_) => eprintln!("✓ Parsed with inputs"),
            Err(e) => eprintln!("✗ Failed to parse with inputs: {e:#?}"),
        }
        
        // Add simple outputs
        let input3 = r#"{
  description = "A Nix-flake-based Go 1.22 development environment";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  outputs = { self, nixpkgs }: {};
}"#;
        match parse_nix_expr(input3) {
            Ok(_) => eprintln!("✓ Parsed with simple outputs"),
            Err(e) => eprintln!("✗ Failed to parse with simple outputs: {e:#?}"),
        }
        
        // Add let binding
        let input4 = r#"{
  description = "A Nix-flake-based Go 1.22 development environment";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  outputs = { self, nixpkgs }: let goVersion = 24; in { };
}"#;
        match parse_nix_expr(input4) {
            Ok(_) => eprintln!("✓ Parsed with let binding"),
            Err(e) => eprintln!("✗ Failed to parse with let binding: {e:#?}"),
        }

        // Add comment in let binding
        let input5 = r#"{
  description = "A Nix-flake-based Go 1.22 development environment";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  outputs = { self, nixpkgs }: let goVersion = 24; # comment
  in { };
}"#;
        match parse_nix_expr(input5) {
            Ok(_) => eprintln!("✓ Parsed with comment in let"),
            Err(e) => eprintln!("✗ Failed to parse with comment in let: {e:#?}"),
        }

        // Add list in let binding
        let input6 = r#"{
  description = "A Nix-flake-based Go 1.22 development environment";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  outputs = { self, nixpkgs }: let 
    goVersion = 24;
    supportedSystems = [ "x86_64-linux" "aarch64-linux" ];
  in { };
}"#;
        match parse_nix_expr(input6) {
            Ok(_) => eprintln!("✓ Parsed with list in let"),
            Err(e) => eprintln!("✗ Failed to parse with list in let: {e:#?}"),
        }

        // Add function definition
        let input7 = r#"{
  description = "A Nix-flake-based Go 1.22 development environment";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  outputs = { self, nixpkgs }: let 
    goVersion = 24;
    supportedSystems = [ "x86_64-linux" ];
    forEachSupportedSystem = f: f;
  in { };
}"#;
        match parse_nix_expr(input7) {
            Ok(_) => eprintln!("✓ Parsed with function definition"),
            Err(e) => eprintln!("✗ Failed to parse with function definition: {e:#?}"),
        }

        // Test the complex function call pattern from go template
        let input8 = r#"{
  outputs = { self, nixpkgs }: let 
    supportedSystems = [ "x86_64-linux" ];
    forEachSupportedSystem = f: nixpkgs.lib.genAttrs supportedSystems f;
  in { };
}"#;
        match parse_nix_expr(input8) {
            Ok(_) => eprintln!("✓ Parsed with complex function call"),
            Err(e) => eprintln!("✗ Failed to parse with complex function call: {e:#?}"),
        }

        // Test with lambda parameter in function call
        let input9 = r#"{
  outputs = { self, nixpkgs }: let 
    supportedSystems = [ "x86_64-linux" ];
    forEachSupportedSystem = f: nixpkgs.lib.genAttrs supportedSystems (system: f);
  in { };
}"#;
        match parse_nix_expr(input9) {
            Ok(_) => eprintln!("✓ Parsed with lambda in function call"),
            Err(e) => eprintln!("✗ Failed to parse with lambda in function call: {e:#?}"),
        }

        // Test with more complex lambda body like in go template
        let input10 = r#"{
  outputs = { self, nixpkgs }: let 
    supportedSystems = [ "x86_64-linux" ];
    forEachSupportedSystem = f: nixpkgs.lib.genAttrs supportedSystems (
      system: f { pkgs = import nixpkgs { inherit system; }; }
    );
  in { };
}"#;
        match parse_nix_expr(input10) {
            Ok(_) => eprintln!("✓ Parsed with complex lambda body"),
            Err(e) => eprintln!("✗ Failed to parse with complex lambda body: {e:#?}"),
        }
    }

    #[test]
    fn test_binary_operator_parsing() {
        let input = r#"a ++ b"#;
        let result = parse_nix_expr(input).unwrap();
        
        match result {
            NixExpr::BinaryOp { left, op, right } => {
                assert_eq!(*left, NixExpr::Identifier("a".to_string()));
                assert_eq!(op, BinaryOperator::Concat);
                assert_eq!(*right, NixExpr::Identifier("b".to_string()));
            }
            _ => panic!("Expected BinaryOp"),
        }
    }

    #[test]
    fn test_with_expression() {
        let input = r#"with pkgs; [ hello ]"#;
        let result = parse_nix_expr(input).unwrap();
        
        match result {
            NixExpr::With { env, body } => {
                assert_eq!(*env, NixExpr::Identifier("pkgs".to_string()));
                match *body {
                    NixExpr::List(items) => {
                        assert_eq!(items.len(), 1);
                        assert_eq!(items[0], NixExpr::Identifier("hello".to_string()));
                    }
                    _ => panic!("Expected List in with body"),
                }
            }
            _ => panic!("Expected With expression"),
        }
    }

    #[test]
    fn test_select_expression() {
        let input = r#"pkgs.hello"#;
        let result = parse_nix_expr(input).unwrap();
        
        match result {
            NixExpr::Select { expr, path, .. } => {
                assert_eq!(*expr, NixExpr::Identifier("pkgs".to_string()));
                assert_eq!(path.parts.len(), 1);
                assert_eq!(path.parts[0], AttrPathPart::Identifier("hello".to_string()));
            }
            _ => panic!("Expected Select expression"),
        }
    }
}