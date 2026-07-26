//! Modelica-like expression pretty-printer for `rumoca_core::Expression`.
//!
//! Renders DAE expressions as readable Modelica text (e.g. `der(w) - tau/J`)
//! instead of opaque labels like `f_x[0]`. The printer is precedence-aware:
//! it inserts parentheses only when needed, producing clean output for typical
//! equations.
//!
//! Entry points:
//! - `format_expr` — render a single `Expression`
//! - `format_equation` — render a DAE `Equation` as `lhs = rhs` or `0 = rhs`

use std::fmt::Write as _;

use rumoca_core::{Expression, OpBinary, Subscript};
use rumoca_ir_dae as dae;

fn precedence(op: &OpBinary) -> u8 {
    match op {
        OpBinary::Or => 1,
        OpBinary::And => 2,
        OpBinary::Eq | OpBinary::Neq | OpBinary::Lt | OpBinary::Le |
        OpBinary::Gt | OpBinary::Ge => 3,
        OpBinary::Add | OpBinary::Sub | OpBinary::AddElem | OpBinary::SubElem => 4,
        OpBinary::Mul | OpBinary::Div | OpBinary::MulElem | OpBinary::DivElem => 5,
        OpBinary::Exp | OpBinary::ExpElem => 6,
        OpBinary::Assign | OpBinary::Empty => 0,
    }
}

fn is_right_associative(op: &OpBinary) -> bool {
    matches!(op, OpBinary::Exp | OpBinary::ExpElem)
}

fn needs_left_parens(child: &Expression, parent_op: &OpBinary) -> bool {
    if let Expression::Binary { op: child_op, .. } = child {
        precedence(child_op) < precedence(parent_op)
    } else {
        false
    }
}

fn needs_right_parens(child: &Expression, parent_op: &OpBinary) -> bool {
    if let Expression::Binary { op: child_op, .. } = child {
        let cp = precedence(child_op);
        let pp = precedence(parent_op);
        if cp < pp {
            return true;
        }
        // Same precedence: parenthesize the right child for left-associative ops
        // (e.g. `a - (b - c)` needs parens, but `a ^ (b ^ c)` doesn't).
        if cp == pp && !is_right_associative(parent_op) {
            return matches!(
                parent_op,
                OpBinary::Sub | OpBinary::SubElem | OpBinary::Div | OpBinary::DivElem
            );
        }
        false
    } else {
        false
    }
}

fn format_subscript(sub: &Subscript, out: &mut String) {
    match sub {
        Subscript::Index { value, .. } => {
            out.push_str(&value.to_string());
        }
        Subscript::Colon { .. } => {
            out.push(':');
        }
        Subscript::Expr { expr, .. } => {
            format_expr_into(expr, out);
        }
    }
}

fn format_expr_into(expr: &Expression, out: &mut String) {
    match expr {
        Expression::Binary { op, lhs, rhs, .. } => {
            let left_parens = needs_left_parens(lhs, op);
            let right_parens = needs_right_parens(rhs, op);

            if left_parens { out.push('('); }
            format_expr_into(lhs, out);
            if left_parens { out.push(')'); }

            // Write the operator directly into the output buffer to avoid
            // an intermediate String allocation on this hot path.
            let before = out.len();
            let _ = write!(out, "{op}");
            if out.len() > before {
                out.insert(before, ' ');
                out.push(' ');
            }

            if right_parens { out.push('('); }
            format_expr_into(rhs, out);
            if right_parens { out.push(')'); }
        }
        Expression::Unary { op, rhs, .. } => {
            let need_parens = matches!(rhs.as_ref(), Expression::Binary { .. });
            let _ = write!(out, "{op}");
            if need_parens { out.push('('); }
            format_expr_into(rhs, out);
            if need_parens { out.push(')'); }
        }
        Expression::VarRef { name, subscripts, .. } => {
            out.push_str(&name.to_string());
            if !subscripts.is_empty() {
                out.push('[');
                for (i, sub) in subscripts.iter().enumerate() {
                    if i > 0 { out.push_str(", "); }
                    format_subscript(sub, out);
                }
                out.push(']');
            }
        }
        Expression::BuiltinCall { function, args, .. } => {
            out.push_str(function.name());
            out.push('(');
            for (i, arg) in args.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                format_expr_into(arg, out);
            }
            out.push(')');
        }
        Expression::FunctionCall { name, args, .. } => {
            out.push_str(&name.to_string());
            out.push('(');
            for (i, arg) in args.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                format_expr_into(arg, out);
            }
            out.push(')');
        }
        Expression::Literal { value, .. } => {
            out.push_str(&value.to_string());
        }
        Expression::If { branches, else_branch, .. } => {
            for (i, (cond, then_expr)) in branches.iter().enumerate() {
                if i == 0 {
                    out.push_str("if ");
                } else {
                    out.push_str(" elseif ");
                }
                format_expr_into(cond, out);
                out.push_str(" then ");
                format_expr_into(then_expr, out);
            }
            out.push_str(" else ");
            format_expr_into(else_branch, out);
        }
        Expression::Array { elements, .. } => {
            out.push('{');
            for (i, e) in elements.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                format_expr_into(e, out);
            }
            out.push('}');
        }
        Expression::Tuple { elements, .. } => {
            out.push('(');
            for (i, e) in elements.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                format_expr_into(e, out);
            }
            out.push(')');
        }
        Expression::Range { start, step, end, .. } => {
            format_expr_into(start, out);
            out.push(':');
            if let Some(s) = step {
                format_expr_into(s, out);
                out.push(':');
            }
            format_expr_into(end, out);
        }
        Expression::ArrayComprehension { expr, indices, filter, .. } => {
            out.push('{');
            format_expr_into(expr, out);
            out.push_str(" for ");
            for (i, idx) in indices.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                out.push_str(&idx.name);
                out.push_str(" in ");
                format_expr_into(&idx.range, out);
            }
            if let Some(f) = filter {
                out.push_str(" if ");
                format_expr_into(f, out);
            }
            out.push('}');
        }
        Expression::Index { base, subscripts, .. } => {
            format_expr_into(base, out);
            out.push('[');
            for (i, sub) in subscripts.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                format_subscript(sub, out);
            }
            out.push(']');
        }
        Expression::FieldAccess { base, field, .. } => {
            format_expr_into(base, out);
            out.push('.');
            out.push_str(field);
        }
        Expression::Empty { .. } => {}
    }
}

/// Render a `rumoca_core::Expression` as Modelica-like text.
pub fn format_expr(expr: &Expression) -> String {
    let mut s = String::new();
    format_expr_into(expr, &mut s);
    s
}

/// Render a DAE equation as `lhs = rhs` (explicit) or `0 = rhs` (residual).
pub fn format_equation(eq: &dae::Equation) -> String {
    let rhs = format_expr(&eq.rhs);
    match &eq.lhs {
        Some(lhs_ref) => format!("{lhs_ref} = {rhs}"),
        None => format!("0 = {rhs}"),
    }
}

/// Render an equation in short form for labels (just the rhs, no `0 =` prefix).
pub fn format_equation_short(eq: &dae::Equation) -> String {
    match &eq.lhs {
        Some(lhs_ref) => {
            let rhs = format_expr(&eq.rhs);
            format!("{lhs_ref} = {rhs}")
        }
        None => format_expr(&eq.rhs),
    }
}

/// Build a vec of human-readable equation labels from DAE equations.
///
/// For each equation index in the incidence matrix, produces a string like
/// `der(w) = tau / J` or `der(phi) - w` (for residual-form equations).
/// Falls back to the EquationRef index label if the DAE equation list is
/// shorter than expected (should not happen in practice).
pub fn equation_labels(equations: &[dae::Equation]) -> Vec<String> {
    equations.iter().map(format_equation_short).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rumoca_core::{BuiltinFunction, Literal, OpBinary, OpUnary, Reference, Span};

    fn span() -> Span { Span::DUMMY }

    fn var(name: &str) -> Expression {
        Expression::VarRef {
            name: Reference::new(name),
            subscripts: vec![],
            span: span(),
        }
    }

    fn lit_real(v: f64) -> Expression {
        Expression::Literal {
            value: Literal::Real(v),
            span: span(),
        }
    }

    fn lit_int(v: i64) -> Expression {
        Expression::Literal {
            value: Literal::Integer(v),
            span: span(),
        }
    }

    fn binary(op: OpBinary, lhs: Expression, rhs: Expression) -> Expression {
        Expression::Binary {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
            span: span(),
        }
    }

    fn unary(op: OpUnary, rhs: Expression) -> Expression {
        Expression::Unary {
            op,
            rhs: Box::new(rhs),
            span: span(),
        }
    }

    fn builtin(f: BuiltinFunction, args: Vec<Expression>) -> Expression {
        Expression::BuiltinCall {
            function: f,
            args,
            span: span(),
        }
    }

    fn der(inner: Expression) -> Expression {
        builtin(BuiltinFunction::Der, vec![inner])
    }

    // --- basic expressions ---

    #[test]
    fn simple_variable() {
        assert_eq!(format_expr(&var("x")), "x");
    }

    #[test]
    fn simple_literal() {
        assert_eq!(format_expr(&lit_real(3.14)), "3.14");
        assert_eq!(format_expr(&lit_int(42)), "42");
    }

    #[test]
    fn simple_addition() {
        let e = binary(OpBinary::Add, var("x"), var("y"));
        assert_eq!(format_expr(&e), "x + y");
    }

    #[test]
    fn precedence_mul_over_add() {
        // x + y * z  →  no parens needed
        let e = binary(OpBinary::Add, var("x"),
            binary(OpBinary::Mul, var("y"), var("z")));
        assert_eq!(format_expr(&e), "x + y * z");
    }

    #[test]
    fn precedence_add_in_mul_needs_parens() {
        // (x + y) * z
        let e = binary(OpBinary::Mul,
            binary(OpBinary::Add, var("x"), var("y")),
            var("z"));
        assert_eq!(format_expr(&e), "(x + y) * z");
    }

    #[test]
    fn subtraction_right_associativity_parens() {
        // a - (b - c) needs parens on the right
        let e = binary(OpBinary::Sub, var("a"),
            binary(OpBinary::Sub, var("b"), var("c")));
        assert_eq!(format_expr(&e), "a - (b - c)");
    }

    #[test]
    fn subtraction_left_no_parens() {
        // (a - b) - c  →  a - b - c  (left-associative, no parens needed)
        let e = binary(OpBinary::Sub,
            binary(OpBinary::Sub, var("a"), var("b")),
            var("c"));
        assert_eq!(format_expr(&e), "a - b - c");
    }

    #[test]
    fn division_right_needs_parens() {
        // a / (b / c)
        let e = binary(OpBinary::Div, var("a"),
            binary(OpBinary::Div, var("b"), var("c")));
        assert_eq!(format_expr(&e), "a / (b / c)");
    }

    #[test]
    fn exponentiation_right_associative() {
        // a ^ b ^ c  →  a ^ b ^ c (right-associative, no parens)
        let e = binary(OpBinary::Exp, var("a"),
            binary(OpBinary::Exp, var("b"), var("c")));
        assert_eq!(format_expr(&e), "a ^ b ^ c");
    }

    #[test]
    fn unary_minus() {
        assert_eq!(format_expr(&unary(OpUnary::Minus, var("x"))), "-x");
    }

    #[test]
    fn unary_minus_of_binop_gets_parens() {
        let e = unary(OpUnary::Minus, binary(OpBinary::Add, var("x"), var("y")));
        assert_eq!(format_expr(&e), "-(x + y)");
    }

    #[test]
    fn der_call() {
        assert_eq!(format_expr(&der(var("w"))), "der(w)");
    }

    #[test]
    fn nested_builtin_calls() {
        let e = builtin(BuiltinFunction::Sin,
            vec![binary(OpBinary::Mul, var("omega"), var("t"))]);
        assert_eq!(format_expr(&e), "sin(omega * t)");
    }

    // --- DAE equations ---

    #[test]
    fn explicit_equation() {
        let eq = dae::Equation {
            lhs: Some(Reference::new("x")),
            rhs: binary(OpBinary::Add, var("a"), var("b")),
            span: span(),
            origin: String::new(),
            scalar_count: 1,
        };
        assert_eq!(format_equation(&eq), "x = a + b");
    }

    #[test]
    fn residual_equation() {
        let eq = dae::Equation {
            lhs: None,
            rhs: binary(OpBinary::Sub, der(var("w")),
                binary(OpBinary::Div, var("tau"), var("J"))),
            span: span(),
            origin: String::new(),
            scalar_count: 1,
        };
        assert_eq!(format_equation(&eq), "0 = der(w) - tau / J");
    }

    #[test]
    fn short_form_residual_omits_zero_prefix() {
        let eq = dae::Equation {
            lhs: None,
            rhs: binary(OpBinary::Sub, der(var("w")),
                binary(OpBinary::Div, var("tau"), var("J"))),
            span: span(),
            origin: String::new(),
            scalar_count: 1,
        };
        assert_eq!(format_equation_short(&eq), "der(w) - tau / J");
    }

    #[test]
    fn equation_labels_from_dae() {
        let eqs = vec![
            dae::Equation {
                lhs: Some(Reference::new("x")),
                rhs: var("y"),
                span: span(),
                origin: String::new(),
                scalar_count: 1,
            },
            dae::Equation {
                lhs: None,
                rhs: binary(OpBinary::Sub, der(var("phi")), var("w")),
                span: span(),
                origin: String::new(),
                scalar_count: 1,
            },
        ];
        let labels = equation_labels(&eqs);
        assert_eq!(labels, vec!["x = y", "der(phi) - w"]);
    }

    // --- edge cases ---

    #[test]
    fn if_expression() {
        let e = Expression::If {
            branches: vec![(
                binary(OpBinary::Gt, var("x"), lit_int(0)),
                var("a"),
            )],
            else_branch: Box::new(var("b")),
            span: span(),
        };
        assert_eq!(format_expr(&e), "if x > 0 then a else b");
    }

    #[test]
    fn array_expression() {
        let e = Expression::Array {
            elements: vec![lit_int(1), lit_int(2), lit_int(3)],
            is_matrix: false,
            span: span(),
        };
        assert_eq!(format_expr(&e), "{1, 2, 3}");
    }

    #[test]
    fn field_access() {
        let e = Expression::FieldAccess {
            base: Box::new(var("motor")),
            field: "torque".to_string(),
            span: span(),
        };
        assert_eq!(format_expr(&e), "motor.torque");
    }

    #[test]
    fn empty_expression() {
        let e = Expression::Empty { span: span() };
        assert_eq!(format_expr(&e), "");
    }

    #[test]
    fn var_with_subscripts() {
        let e = Expression::VarRef {
            name: Reference::new("x"),
            subscripts: vec![Subscript::index(1, span()), Subscript::index(2, span())],
            span: span(),
        };
        assert_eq!(format_expr(&e), "x[1, 2]");
    }

    #[test]
    fn range_expression() {
        let e = Expression::Range {
            start: Box::new(lit_int(1)),
            step: None,
            end: Box::new(lit_int(10)),
            span: span(),
        };
        assert_eq!(format_expr(&e), "1:10");
    }

    #[test]
    fn range_with_step() {
        let e = Expression::Range {
            start: Box::new(lit_int(1)),
            step: Some(Box::new(lit_int(2))),
            end: Box::new(lit_int(10)),
            span: span(),
        };
        assert_eq!(format_expr(&e), "1:2:10");
    }

    #[test]
    fn not_operator() {
        let e = unary(OpUnary::Not, var("b"));
        assert_eq!(format_expr(&e), "not b");
    }

    #[test]
    fn logical_operators() {
        // x > 0 and y < 10
        let e = binary(OpBinary::And,
            binary(OpBinary::Gt, var("x"), lit_int(0)),
            binary(OpBinary::Lt, var("y"), lit_int(10)));
        assert_eq!(format_expr(&e), "x > 0 and y < 10");
    }
}
