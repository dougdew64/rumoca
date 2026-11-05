//! Modelica-like pretty-printer for a **0.10.0 DAE's arena expressions**.
//!
//! # Why this exists beside `expr_format`
//!
//! Rumoca has two expression representations and v0.10.0 made them genuinely
//! different things. The flat IR still uses `rumoca_core::Expression`, a
//! recursive tree, and [`crate::expr_format`] renders that. The **DAE** moved
//! onto an arena: an equation holds a [`dae::ExprId`] into a node table, nodes
//! are reached as [`dae::ExpressionView`], and there is no conversion back to a
//! tree anywhere in the workspace. So the two renderers are not duplication —
//! they are two IRs, and merging them would mean inventing a shared tree that
//! neither phase actually has.
//!
//! # Why HRW renders this itself instead of asking Rumoca
//!
//! Upstream has no equation-to-text renderer. Its own
//! `rumoca_phase_structural::equation_label` sidesteps rendering by looking the
//! **source text** back up through the equation's provenance. **HRW must not
//! copy that**, and the reason is the whole point of the equation sheet: a
//! flattened DAE equation is deliberately *not* the source. `resistor.v =
//! resistor.R * resistor.i` sits where the model said `connect(…)`. Printing
//! provenance text there would show Doug the source he already has open and
//! label it as the DAE equation — a false claim in exactly the place built to
//! prevent one.
//!
//! # Where the names come from
//!
//! Builtin names are **not** written out from memory of the specification.
//! `rumoca_phase_dae`'s `pure_builtin` maps `rumoca_core::BuiltinFunction` to
//! [`dae::PureBuiltin`] one-to-one for 41 of the 44 variants, and
//! `BuiltinFunction::name()` is public, so [`builtin_call_name`] inverts that
//! map and asks Rumoca for the spelling. An upstream rename follows
//! automatically, and a variant that stops existing fails to compile here.
//!
//! # Anything with no Modelica spelling says so
//!
//! Three DAE builtins and several coordinate kinds have no surface syntax —
//! they are things the compiler made. Those render inside angle brackets, which
//! is upstream's own convention for the same situation (`unknown_label` emits
//! `<unmatched f_x[0]>`). **A marker that cannot be mistaken for Modelica is the
//! point**: the alternative is a plausible-looking rendering of something the
//! modeller never wrote, which is unrecoverable, where a visibly-internal
//! marker merely asks a question.
//!
//! Every `match` over a Rumoca enum here is exhaustive, so a variant added
//! upstream stops the build instead of falling into a default that looks right.

use std::fmt::Write as _;

use rumoca_core::BuiltinFunction;
use rumoca_ir_dae as dae;

/// Render one residual equation as `0 = <expr>`.
///
/// **Every DAE equation is a residual on 0.10.0**, and that is a real change
/// rather than a rendering choice. The old `dae::Equation` carried `lhs:
/// Option<ComponentReference>`, so HRW printed `x = expr` when an equation had
/// been solved for a variable and `0 = expr` otherwise. A 0.10.0
/// [`dae::ResidualEquationView`] holds only `residual` and `provenance` — the
/// DAE no longer records which variable an equation was solved for, because
/// normalisation to residual form happens before this point. So the `x = expr`
/// form is not suppressed here; there is nothing left to reconstruct it from,
/// and printing one would be inventing the solved-for variable.
pub fn format_equation<'dae>(
    view: dae::DaeView<'dae>,
    equation: dae::ResidualEquationView<'dae>,
) -> String {
    format!("0 = {}", format_expr(view, equation.residual()))
}

/// Render one arena expression as Modelica-like text.
pub fn format_expr<'dae>(view: dae::DaeView<'dae>, id: dae::ExprId<'dae>) -> String {
    let mut out = String::new();
    write_expr(view, id, 0, &mut out);
    out
}

/// Binding power of a binary operator, used only to decide parentheses.
///
/// The ordering follows MLS §3.4's precedence table; the absolute numbers mean
/// nothing beyond their comparisons. Elementwise operators bind exactly as
/// their scalar counterparts do.
fn precedence(operator: dae::BinaryOperator) -> u8 {
    use dae::BinaryOperator as B;
    match operator {
        B::Or => 1,
        B::And => 2,
        B::Equal | B::NotEqual | B::Less | B::LessEqual | B::Greater | B::GreaterEqual => 3,
        B::Add | B::Subtract | B::ElementwiseAdd | B::ElementwiseSubtract => 4,
        B::Multiply | B::Divide | B::ElementwiseMultiply | B::ElementwiseDivide => 5,
        B::Power | B::ElementwisePower => 6,
    }
}

/// The operator's Modelica spelling, with the spaces it is written with.
fn binary_spelling(operator: dae::BinaryOperator) -> &'static str {
    use dae::BinaryOperator as B;
    match operator {
        B::Add => " + ",
        B::Subtract => " - ",
        B::Multiply => "*",
        B::Divide => "/",
        B::Power => "^",
        B::ElementwiseAdd => " .+ ",
        B::ElementwiseSubtract => " .- ",
        B::ElementwiseMultiply => ".*",
        B::ElementwiseDivide => "./",
        B::ElementwisePower => ".^",
        B::Equal => " == ",
        B::NotEqual => " <> ",
        B::Less => " < ",
        B::LessEqual => " <= ",
        B::Greater => " > ",
        B::GreaterEqual => " >= ",
        B::And => " and ",
        B::Or => " or ",
    }
}

/// Whether `a op (b op c)` differs from `(a op b) op c`, which decides whether
/// an equal-precedence right operand needs brackets.
///
/// `a - (b - c)` and `a / (b / c)` are the cases that go wrong without this, and
/// they are common in a residual: dropping the brackets silently changes the
/// equation's value rather than its appearance.
fn is_associative(operator: dae::BinaryOperator) -> bool {
    use dae::BinaryOperator as B;
    matches!(
        operator,
        B::Add
            | B::Multiply
            | B::ElementwiseAdd
            | B::ElementwiseMultiply
            | B::And
            | B::Or
            | B::Equal
            | B::NotEqual
    )
}

/// Write `id` into `out`, bracketing it if its own operator binds more loosely
/// than `floor`.
///
/// `floor` is the binding power the enclosing context demands: 0 for a
/// top-level expression, the parent operator's precedence for a left operand,
/// and one more than that for the right operand of a non-associative operator.
fn write_expr<'dae>(view: dae::DaeView<'dae>, id: dae::ExprId<'dae>, floor: u8, out: &mut String) {
    let Some(expression) = view.expression(id) else {
        // An id that does not resolve is a bug in whoever produced it, not a
        // renderable expression. Say which id, rather than printing nothing and
        // leaving an equation that silently lost a term.
        let _ = write!(out, "<unresolved expression {}>", id.index());
        return;
    };

    match expression.operation() {
        dae::ExpressionOperation::Binary { operator, lhs, rhs } => {
            let own = precedence(operator);
            let bracket = own < floor;
            if bracket {
                out.push('(');
            }
            write_expr(view, lhs, own, out);
            out.push_str(binary_spelling(operator));
            // A non-associative operator's right operand needs brackets even at
            // equal precedence: `a - (b - c)`.
            let right_floor = if is_associative(operator) {
                own
            } else {
                own + 1
            };
            write_expr(view, rhs, right_floor, out);
            if bracket {
                out.push(')');
            }
        }
        dae::ExpressionOperation::Unary { operator, operand } => {
            // Unary binds tighter than every binary operator, so its operand
            // takes the highest floor; `-(a + b)` keeps its brackets.
            match operator {
                dae::UnaryOperator::Plus => out.push('+'),
                dae::UnaryOperator::Negate => out.push('-'),
                dae::UnaryOperator::Not => out.push_str("not "),
            }
            write_expr(view, operand, u8::MAX, out);
        }
        dae::ExpressionOperation::Literal(literal) => write_literal(literal, out),
        dae::ExpressionOperation::Coordinate(coordinate) => {
            write_coordinate(view, coordinate, out);
        }
        // Operands are packed `[cond, value, cond, value, …, fallback]` — read
        // off the `conditional` builder, which flat-maps the branch pairs and
        // then chains the fallback, so the count is always odd.
        dae::ExpressionOperation::Conditional(operands) => {
            let ids: Vec<_> = operands.iter().collect();
            let Some((fallback, branches)) = ids.split_last() else {
                out.push_str("<conditional with no operands>");
                return;
            };
            for (index, pair) in branches.chunks(2).enumerate() {
                let [condition, value] = pair else {
                    // An even operand count means the packing assumption above
                    // no longer holds. Report it rather than render half a
                    // branch as though it were whole.
                    out.push_str("<conditional with an unpaired branch>");
                    return;
                };
                out.push_str(if index == 0 { "if " } else { " elseif " });
                write_expr(view, *condition, 0, out);
                out.push_str(" then ");
                write_expr(view, *value, 0, out);
            }
            out.push_str(" else ");
            write_expr(view, *fallback, 0, out);
        }
        // Element construction only. Matrix syntax never reaches the DAE as an
        // array: `[a, b]` and `[a; b]` are lowered to the promoted
        // concatenation builtins, which is why there is no `kind` here and why
        // braces are unconditionally correct.
        dae::ExpressionOperation::Array(operands) => {
            out.push('{');
            write_separated(view, operands, ", ", out);
            out.push('}');
        }
        // Modelica has no anonymous record-literal syntax, so this is marked
        // rather than dressed as a constructor call with a name it does not
        // have. Field *reads* stay readable through the `Field` arm below.
        dae::ExpressionOperation::Record(operands) => {
            out.push_str("<record>(");
            write_separated(view, operands, ", ", out);
            out.push(')');
        }
        dae::ExpressionOperation::Field { base, field } => {
            write_expr(view, base, u8::MAX, out);
            out.push('.');
            // The field name lives on the base's record type, reached through
            // the base expression's own value type. Falling back to the ordinal
            // keeps the access visible when the type does not resolve.
            let name = view
                .expression(base)
                .and_then(|base| view.record_field(base.value_type_id(), field as usize))
                .map(|(name, _)| name.to_string());
            match name {
                Some(name) => out.push_str(&name),
                None => {
                    let _ = write!(out, "<field {field}>");
                }
            }
        }
        dae::ExpressionOperation::Range(range) => {
            write_expr(view, range.start().expression(), u8::MAX, out);
            if let Some(step) = range.explicit_step() {
                out.push(':');
                write_expr(view, step.expression(), u8::MAX, out);
            }
            out.push(':');
            write_expr(view, range.stop().expression(), u8::MAX, out);
        }
        dae::ExpressionOperation::Index { base, subscripts } => {
            write_expr(view, base, u8::MAX, out);
            out.push('[');
            for (index, subscript) in subscripts.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                match subscript {
                    dae::SubscriptView::Index { expression, .. }
                    | dae::SubscriptView::Slice { expression, .. } => {
                        write_expr(view, expression, 0, out);
                    }
                    dae::SubscriptView::Whole { .. } => out.push(':'),
                }
            }
            out.push(']');
        }
        // A functional array update — the compiler's way of writing "this array,
        // but with these elements replaced". No Modelica expression does this,
        // so it is marked, with `:=` naming the assignment it stands for.
        dae::ExpressionOperation::ArrayUpdate {
            base,
            value,
            subscripts,
        } => {
            out.push_str("<update ");
            write_expr(view, base, u8::MAX, out);
            out.push('[');
            for (index, subscript) in subscripts.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                match subscript {
                    dae::SubscriptView::Index { expression, .. }
                    | dae::SubscriptView::Slice { expression, .. } => {
                        write_expr(view, expression, 0, out);
                    }
                    dae::SubscriptView::Whole { .. } => out.push(':'),
                }
            }
            out.push_str("] := ");
            write_expr(view, value, 0, out);
            out.push('>');
        }
        dae::ExpressionOperation::Builtin { builtin, arguments } => {
            match builtin_call_name(builtin) {
                BuiltinCall::Named(name) => {
                    out.push_str(name);
                    out.push('(');
                    write_separated(view, arguments, ", ", out);
                    out.push(')');
                }
                // `cat`'s dimension is carried by the variant rather than by an
                // operand, so it is written back as the first argument — which
                // is where the modeller wrote it.
                BuiltinCall::Concatenation { dimension } => {
                    let _ = write!(out, "cat({dimension}");
                    for argument in arguments.iter() {
                        out.push_str(", ");
                        write_expr(view, argument, 0, out);
                    }
                    out.push(')');
                }
                BuiltinCall::Internal(name) => {
                    let _ = write!(out, "<{name}>(");
                    write_separated(view, arguments, ", ", out);
                    out.push(')');
                }
            }
        }
        dae::ExpressionOperation::Call {
            function,
            arguments,
            ..
        } => {
            match view
                .function(function)
                .map(|function| function.name().to_string())
            {
                Some(name) => out.push_str(&name),
                None => out.push_str("<unresolved function>"),
            }
            out.push('(');
            write_separated(view, arguments, ", ", out);
            out.push(')');
        }
        // The predefined `String` conversion of MLS §3.7.1, which 0.10.0 keeps
        // distinct from a user call so the resolved overload stays explicit.
        // Only the arguments actually present are written: a rendering that
        // dropped them would show a conversion discarding its own formatting.
        dae::ExpressionOperation::StringConversion { value, format, .. } => {
            out.push_str("String(");
            write_expr(view, value, 0, out);
            match format {
                dae::StringConversionFormatView::Options {
                    minimum_length,
                    left_justified,
                    significant_digits,
                } => {
                    for (name, argument) in [
                        ("minimumLength", minimum_length),
                        ("leftJustified", left_justified),
                        ("significantDigits", significant_digits),
                    ] {
                        if let Some(argument) = argument {
                            let _ = write!(out, ", {name} = ");
                            write_expr(view, argument, 0, out);
                        }
                    }
                }
                dae::StringConversionFormatView::Format { value } => {
                    out.push_str(", format = ");
                    write_expr(view, value, 0, out);
                }
            }
            out.push(')');
        }
        // An array comprehension's iteration domain is a separate arena entry
        // and its binders are not reachable from here, so the domain is named by
        // ordinal. The body stays fully rendered, which is the part that says
        // what the comprehension computes.
        dae::ExpressionOperation::Comprehension { domain, body } => {
            out.push('{');
            write_expr(view, body, 0, out);
            let _ = write!(out, " for <domain {}>}}", domain.index());
        }
        // Clocked-partition transfer (`sample`, `hold`, and the sub/super-sample
        // family). The kind is carried as an enum rather than a name, so it is
        // reported as the compiler's own identifier instead of being guessed
        // into one of several similar Modelica operators.
        dae::ExpressionOperation::ClockTransfer { kind, source, .. } => {
            let _ = write!(out, "<clock {kind:?}>(");
            write_expr(view, source, 0, out);
            out.push(')');
        }
        // The four remaining kinds exist only inside a lowered function body —
        // a local value, a fold's carried input and output, and (new in 0.10.4)
        // one output of an ordered conditional. They have no model-level
        // spelling at all.
        // These ids are owner-local: a function or fold plus an ordinal within
        // it, so both halves are printed. An ordinal alone would collide across
        // functions and read as though two unrelated locals were the same one.
        dae::ExpressionOperation::FunctionValue { value, .. } => {
            let _ = write!(
                out,
                "<function {} value {}>",
                value.function().index(),
                value.ordinal()
            );
        }
        dae::ExpressionOperation::FunctionFoldParameter { fold, carried, .. } => {
            let _ = write!(
                out,
                "<function {} fold {} carried in {carried}>",
                fold.function().index(),
                fold.ordinal()
            );
        }
        dae::ExpressionOperation::FunctionFoldOutput { fold, carried, .. } => {
            let _ = write!(
                out,
                "<function {} fold {} carried out {carried}>",
                fold.function().index(),
                fold.ordinal()
            );
        }
        // **Two owner-local ordinals, not one.** A conditional's output is
        // identified by *which* conditional and *which* of its values, and both
        // are local to the same function, so printing either alone would read as
        // though unrelated outputs were the same one. `definition` is skipped
        // because it names the same function the ids already carry.
        dae::ExpressionOperation::FunctionConditionalOutput {
            conditional, value, ..
        } => {
            let _ = write!(
                out,
                "<function {} conditional {} value {}>",
                conditional.function().index(),
                conditional.ordinal(),
                value.ordinal()
            );
        }
    }
}

/// Write every operand, separated, each at the lowest binding power.
///
/// Arguments and elements are already delimited by the brackets around them, so
/// nothing inside needs defensive parentheses.
fn write_separated<'dae>(
    view: dae::DaeView<'dae>,
    operands: dae::ExpressionOperands<'dae>,
    separator: &str,
    out: &mut String,
) {
    for (index, operand) in operands.iter().enumerate() {
        if index > 0 {
            out.push_str(separator);
        }
        write_expr(view, operand, 0, out);
    }
}

fn write_literal(literal: &dae::DaeLiteral, out: &mut String) {
    match literal {
        dae::DaeLiteral::Real(value) => {
            let _ = write!(out, "{value}");
        }
        dae::DaeLiteral::Integer(value) => {
            let _ = write!(out, "{value}");
        }
        // An enumeration literal is stored as its ordinal; the type that would
        // name it is not carried on the node, so the ordinal is marked rather
        // than printed as a bare integer it could be confused with.
        dae::DaeLiteral::Enumeration(value) => {
            let _ = write!(out, "<enumeration {value}>");
        }
        dae::DaeLiteral::Boolean(value) => {
            let _ = write!(out, "{value}");
        }
        dae::DaeLiteral::String(value) => {
            let _ = write!(out, "{value:?}");
        }
    }
}

/// Write the name a coordinate reads.
///
/// The six variable roles that carry a [`dae::VariableId`] print the declared
/// name. `Derivative` and the four `Pre*` kinds wrap it in the operator they
/// stand for, because `der(w)` and `w` are different quantities and a renderer
/// that printed both as `w` would make a differentiated equation unreadable.
/// The rest name compiler-made entries with no declaration to look up.
fn write_coordinate<'dae>(
    view: dae::DaeView<'dae>,
    coordinate: dae::CoordinateView<'dae>,
    out: &mut String,
) {
    use dae::CoordinateView as C;

    // Resolve a variable identity to its declared name, keeping the identity
    // visible if it does not resolve — a silent fallback would turn a broken
    // reference into a plausible equation.
    let named = |id: dae::VariableId<'dae>, out: &mut String| match view.variable(id) {
        Some(variable) => out.push_str(&variable.name().to_string()),
        None => {
            let _ = write!(out, "<unresolved variable {}>", id.index());
        }
    };
    let wrapped = |operator: &str, id: dae::VariableId<'dae>, out: &mut String| {
        out.push_str(operator);
        out.push('(');
        named(id, out);
        out.push(')');
    };

    match coordinate {
        C::Parameter(id) => named(id.into(), out),
        C::Input(id) => named(id.into(), out),
        C::State(id) => named(id.into(), out),
        C::Algebraic(id) => named(id.into(), out),
        C::DiscreteReal(id) => named(id.into(), out),
        C::DiscreteValue(id) => named(id.into(), out),
        C::Derivative(id) => wrapped("der", id.into(), out),
        C::PreState(id) => wrapped("pre", id.into(), out),
        C::PreAlgebraic(id) => wrapped("pre", id.into(), out),
        C::PreDiscreteReal(id) => wrapped("pre", id.into(), out),
        C::PreDiscreteValue(id) => wrapped("pre", id.into(), out),
        C::Time => out.push_str("time"),
        C::ClockInterval(id) => {
            let _ = write!(out, "interval(<clock {}>)", id.index());
        }
        C::Condition(id) => {
            let _ = write!(out, "<condition {}>", id.index());
        }
        C::Delay(id) => {
            let _ = write!(out, "<delay {}>", id.index());
        }
        C::Previous(id) => {
            let _ = write!(out, "<previous {}>", id.index());
        }
        C::Terminal(id) => {
            let _ = write!(out, "<terminal {}>", id.index());
        }
        // Owner-local ids again: an iteration binder belongs to a domain and a
        // function parameter to a function, so both halves are printed.
        C::Binder(id) => {
            let _ = write!(
                out,
                "<domain {} binder {}>",
                id.domain().index(),
                id.ordinal()
            );
        }
        C::FunctionParameter(id) => {
            let _ = write!(
                out,
                "<function {} parameter {}>",
                id.function().index(),
                id.ordinal()
            );
        }
    }
}

/// How one DAE builtin is written.
enum BuiltinCall {
    /// A Modelica builtin, spelled by Rumoca's own name table.
    Named(&'static str),
    /// Promoted concatenation, whose dimension is in the variant rather than in
    /// an operand.
    Concatenation { dimension: u8 },
    /// No Modelica spelling exists; the name is the compiler's.
    Internal(&'static str),
}

/// Invert `rumoca_phase_dae`'s `pure_builtin` map and ask Rumoca for the name.
///
/// **The 41 names are not written out here on purpose.** They come from
/// `BuiltinFunction::name()`, Rumoca's own table, so HRW cannot drift from the
/// compiler's spelling and an upstream rename arrives for free. Writing them by
/// hand would put 41 chances to be plausibly wrong into the one place a reader
/// has no way to check.
///
/// Three variants have no `BuiltinFunction` counterpart at all, which is why
/// this returns an enum rather than a string:
///
/// - `PromotedCat1` / `PromotedCat2` are what matrix syntax lowers to. Their
///   dimension is Modelica's own 1-based `cat` dimension — read from
///   `rumoca-phase-dae`'s construction site, which maps `1 => PromotedCat1` and
///   `2 => PromotedCat2`, not inferred from the variant names.
/// - `LinearSolve` is produced by solve lowering and has no model-level syntax.
fn builtin_call_name(builtin: dae::PureBuiltin) -> BuiltinCall {
    use dae::PureBuiltin as P;

    let function = match builtin {
        P::Abs => BuiltinFunction::Abs,
        P::Sign => BuiltinFunction::Sign,
        P::Sqrt => BuiltinFunction::Sqrt,
        P::Div => BuiltinFunction::Div,
        P::Mod => BuiltinFunction::Mod,
        P::Rem => BuiltinFunction::Rem,
        P::Floor => BuiltinFunction::Floor,
        P::Ceil => BuiltinFunction::Ceil,
        P::Integer => BuiltinFunction::Integer,
        P::Sin => BuiltinFunction::Sin,
        P::Cos => BuiltinFunction::Cos,
        P::Tan => BuiltinFunction::Tan,
        P::Asin => BuiltinFunction::Asin,
        P::Acos => BuiltinFunction::Acos,
        P::Atan => BuiltinFunction::Atan,
        P::Atan2 => BuiltinFunction::Atan2,
        P::Sinh => BuiltinFunction::Sinh,
        P::Cosh => BuiltinFunction::Cosh,
        P::Tanh => BuiltinFunction::Tanh,
        P::Exp => BuiltinFunction::Exp,
        P::Log => BuiltinFunction::Log,
        P::Log10 => BuiltinFunction::Log10,
        P::Smooth => BuiltinFunction::Smooth,
        P::NoEvent => BuiltinFunction::NoEvent,
        P::Homotopy => BuiltinFunction::Homotopy,
        P::Min => BuiltinFunction::Min,
        P::Max => BuiltinFunction::Max,
        P::Sum => BuiltinFunction::Sum,
        P::Product => BuiltinFunction::Product,
        P::Size => BuiltinFunction::Size,
        P::Zeros => BuiltinFunction::Zeros,
        P::Ones => BuiltinFunction::Ones,
        P::Fill => BuiltinFunction::Fill,
        P::Linspace => BuiltinFunction::Linspace,
        P::Cross => BuiltinFunction::Cross,
        P::Identity => BuiltinFunction::Identity,
        P::Vector => BuiltinFunction::Vector,
        P::Transpose => BuiltinFunction::Transpose,
        P::Diagonal => BuiltinFunction::Diagonal,
        P::OuterProduct => BuiltinFunction::OuterProduct,
        P::Skew => BuiltinFunction::Skew,
        P::PromotedCat1 => return BuiltinCall::Concatenation { dimension: 1 },
        P::PromotedCat2 => return BuiltinCall::Concatenation { dimension: 2 },
        P::LinearSolve => return BuiltinCall::Internal("linearSolve"),
    };
    BuiltinCall::Named(function.name())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rumoca_core::{SourceMap, Span, TypeId, VarName};

    /// A DAE over `Real w, tau, J` with one residual per case under test.
    ///
    /// Everything goes through `Dae::construct`, so these are expressions the
    /// real arena accepted — a fixture that type-checked but could not actually
    /// be built would prove nothing about what this renderer sees at runtime.
    fn rendered(
        build: impl for<'a, 'storage, 'dae> FnOnce(&mut Builder<'a, 'storage, 'dae>),
    ) -> Vec<String> {
        let source_text = "Real w; Real tau; parameter Real J = 2.0;";
        let mut sources = SourceMap::new();
        let sid = sources.add("test://render.mo", source_text);
        let at = dae::DaeProvenance::source(Span::from_offsets(sid, 0, 7))
            .expect("fixture span lies inside the fixture source");

        let dae = dae::Dae::construct(sources, |model| {
            let real = model.types(|types| {
                types.intern(
                    TypeId::new(0),
                    dae::ValueType::scalar(dae::ScalarType::Real),
                    at,
                )
            })?;
            let (w, tau, j) = model.variables(|variables| {
                Ok((
                    variables.state(
                        VarName::new("w"),
                        real,
                        at,
                        dae::VariableAttributes::default(),
                    )?,
                    variables.algebraic(
                        VarName::new("tau"),
                        real,
                        at,
                        dae::VariableAttributes::default(),
                    )?,
                    variables.parameter(
                        VarName::new("J"),
                        real,
                        at,
                        dae::VariableAttributes::default(),
                    )?,
                ))
            })?;
            let residuals = model.expressions(|expressions| {
                let mut builder = Builder {
                    expressions,
                    at,
                    w,
                    tau,
                    j,
                    residuals: Vec::new(),
                };
                build(&mut builder);
                Ok(builder.residuals)
            })?;
            model.continuous(|continuous| {
                for residual in residuals {
                    continuous.equation(at, |equation| equation.residual(residual))?;
                }
                Ok(())
            })
        })
        .expect("fixture DAE is well-formed");

        dae.inspect(|view| {
            (0..view.continuous_equation_count())
                .map(|index| {
                    let equation = view
                        .continuous_equation(index)
                        .expect("index below the equation count resolves");
                    format_equation(view, equation)
                })
                .collect()
        })
    }

    /// Scratch state for a fixture body: the arena plus the fixture's variables.
    ///
    /// The borrow (`'a`) and the arena's own storage lifetime (`'storage`) are
    /// separate parameters on purpose. Writing both as one `'a` makes the field
    /// a `&'a mut Expressions<'a, _>`, and because a mutable reference is
    /// invariant over its target the two lifetimes then have to be equal rather
    /// than merely compatible — which no caller can satisfy.
    struct Builder<'a, 'storage, 'dae> {
        expressions: &'a mut dae::Expressions<'storage, 'dae>,
        at: dae::DaeProvenance,
        w: dae::StateId<'dae>,
        tau: dae::AlgebraicId<'dae>,
        j: dae::ParameterId<'dae>,
        residuals: Vec<dae::ExprId<'dae>>,
    }

    /// `der(w) - tau/J` renders as the equation a modeller would recognise, and
    /// the division needs no brackets because it binds tighter than the
    /// subtraction.
    #[test]
    fn a_residual_renders_as_modelica() {
        let out = rendered(|b| {
            let dw = b.derivative(b.w);
            let tau = b.algebraic(b.tau);
            let j = b.parameter(b.j);
            let quotient = b.binary(dae::BinaryOperator::Divide, tau, j);
            let residual = b.binary(dae::BinaryOperator::Subtract, dw, quotient);
            b.residuals.push(residual);
        });
        assert_eq!(out, vec!["0 = der(w) - tau/J"]);
    }

    /// Brackets appear exactly where removing them would change the value.
    ///
    /// The second case is the one worth having a test for: `tau - (tau - J)` is
    /// not `tau - tau - J`, and a renderer that dropped those brackets would
    /// show a different equation from the one the DAE holds, while still looking
    /// like well-formed Modelica.
    #[test]
    fn brackets_appear_only_where_they_change_the_value() {
        let out = rendered(|b| {
            let tau = b.algebraic(b.tau);
            let j = b.parameter(b.j);

            // (tau + J)*J — the sum must keep its brackets inside a product.
            let sum = b.binary(dae::BinaryOperator::Add, tau, j);
            let product = b.binary(dae::BinaryOperator::Multiply, sum, j);
            b.residuals.push(product);

            // tau - (tau - J) — a non-associative operator on the right.
            let inner = b.binary(dae::BinaryOperator::Subtract, tau, j);
            let outer = b.binary(dae::BinaryOperator::Subtract, tau, inner);
            b.residuals.push(outer);

            // tau + (tau + J) — associative, so the brackets are not needed.
            let inner = b.binary(dae::BinaryOperator::Add, tau, j);
            let outer = b.binary(dae::BinaryOperator::Add, tau, inner);
            b.residuals.push(outer);
        });
        assert_eq!(
            out,
            vec![
                "0 = (tau + J)*J",
                "0 = tau - (tau - J)",
                "0 = tau + tau + J",
            ]
        );
    }

    /// Builtin names come from Rumoca's own table, and `time` and `pre` render
    /// as the operators they are rather than as the variable they wrap.
    #[test]
    fn builtins_and_operator_coordinates_render_by_name() {
        let out = rendered(|b| {
            let tau = b.algebraic(b.tau);
            let sine = b.builtin(dae::PureBuiltin::Sin, [tau]);
            b.residuals.push(sine);

            let time = b.coordinate(dae::CoordinateInput::Time);
            let no_event = b.builtin(dae::PureBuiltin::NoEvent, [time]);
            b.residuals.push(no_event);

            let previous = b.coordinate(dae::CoordinateInput::PreState(b.w));
            b.residuals.push(previous);
        });
        assert_eq!(out, vec!["0 = sin(tau)", "0 = noEvent(time)", "0 = pre(w)"]);
    }

    /// A conditional's operands are packed `[cond, value, …, fallback]`, and an
    /// `elseif` chain is reconstructed from that packing.
    ///
    /// This pins the packing itself, which was read off the `conditional`
    /// builder rather than assumed: were the pairs ordered `[value, cond]`
    /// instead, every branch would render with its test and result swapped and
    /// still read as valid Modelica.
    #[test]
    fn a_conditional_reconstructs_its_branches() {
        let out = rendered(|b| {
            let tau = b.algebraic(b.tau);
            let j = b.parameter(b.j);
            let zero = b.literal(dae::DaeLiteral::Real(0.0));
            let first = b.binary(dae::BinaryOperator::Greater, tau, zero);
            let second = b.binary(dae::BinaryOperator::Less, tau, zero);
            let conditional = b.conditional([(first, j), (second, tau)], zero);
            b.residuals.push(conditional);
        });
        assert_eq!(
            out,
            vec!["0 = if tau > 0 then J elseif tau < 0 then tau else 0"]
        );
    }

    impl<'dae> Builder<'_, '_, 'dae> {
        fn coordinate(&mut self, input: dae::CoordinateInput<'dae>) -> dae::ExprId<'dae> {
            self.expressions
                .at(self.at)
                .coordinate(input)
                .expect("fixture coordinate is well-formed")
        }

        fn derivative(&mut self, state: dae::StateId<'dae>) -> dae::ExprId<'dae> {
            self.coordinate(dae::CoordinateInput::Derivative(state))
        }

        fn algebraic(&mut self, variable: dae::AlgebraicId<'dae>) -> dae::ExprId<'dae> {
            self.coordinate(dae::CoordinateInput::Algebraic(variable))
        }

        fn parameter(&mut self, variable: dae::ParameterId<'dae>) -> dae::ExprId<'dae> {
            self.coordinate(dae::CoordinateInput::Parameter(variable))
        }

        fn literal(&mut self, value: dae::DaeLiteral) -> dae::ExprId<'dae> {
            self.expressions
                .at(self.at)
                .literal(value)
                .expect("fixture literal is well-formed")
        }

        fn binary(
            &mut self,
            operator: dae::BinaryOperator,
            lhs: dae::ExprId<'dae>,
            rhs: dae::ExprId<'dae>,
        ) -> dae::ExprId<'dae> {
            self.expressions
                .at(self.at)
                .binary(operator, lhs, rhs)
                .expect("fixture binary operation is well-formed")
        }

        fn builtin(
            &mut self,
            builtin: dae::PureBuiltin,
            arguments: impl IntoIterator<Item = dae::ExprId<'dae>>,
        ) -> dae::ExprId<'dae> {
            self.expressions
                .at(self.at)
                .builtin(builtin, arguments)
                .expect("fixture builtin is well-formed")
        }

        fn conditional(
            &mut self,
            branches: impl IntoIterator<Item = (dae::ExprId<'dae>, dae::ExprId<'dae>)>,
            fallback: dae::ExprId<'dae>,
        ) -> dae::ExprId<'dae> {
            self.expressions
                .at(self.at)
                .conditional(branches, fallback)
                .expect("fixture conditional is well-formed")
        }
    }
}
