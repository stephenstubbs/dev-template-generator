use crate::ast::*;
use std::collections::HashMap;

pub fn extract_flake_data(expr: &NixExpr) -> Result<FlakeData, ParseError> {
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

pub fn extract_fragments_from_expr(expr: &NixExpr) -> Result<FlakeFragments, ParseError> {
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