use crate::{
    error::ok,
    semantic_analysis::{
        IsConstant, TypeCheckContext, TypedExpression, TypedExpressionVariant,
        TypedVariableDeclaration, VariableMutability,
    },
    type_engine::*,
    CompileResult, FunctionParameter, Ident, TypedDeclaration,
};

use sway_types::{span::Span, Spanned};

#[derive(Debug, Clone, Eq)]
pub struct TypedFunctionParameter {
    pub name: Ident,
    pub is_mutable: bool,
    pub type_id: TypeId,
    pub type_span: Span,
}

// NOTE: Hash and PartialEq must uphold the invariant:
// k1 == k2 -> hash(k1) == hash(k2)
// https://doc.rust-lang.org/std/collections/struct.HashMap.html
impl PartialEq for TypedFunctionParameter {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && look_up_type_id(self.type_id) == look_up_type_id(other.type_id)
            && self.is_mutable == other.is_mutable
    }
}

impl CopyTypes for TypedFunctionParameter {
    fn copy_types(&mut self, type_mapping: &TypeMapping) {
        self.type_id.update_type(type_mapping, &self.type_span);
    }
}

impl TypedFunctionParameter {
    pub fn is_self(&self) -> bool {
        self.name.as_str() == "self"
    }

    pub(crate) fn type_check(
        mut ctx: TypeCheckContext,
        parameter: FunctionParameter,
    ) -> CompileResult<Self> {
        let mut warnings = vec![];
        let mut errors = vec![];
        let type_id = check!(
            ctx.resolve_type_with_self(
                parameter.type_id,
                &parameter.type_span,
                EnforceTypeArguments::Yes,
                None
            ),
            insert_type(TypeInfo::ErrorRecovery),
            warnings,
            errors,
        );
        ctx.namespace.insert_symbol(
            parameter.name.clone(),
            TypedDeclaration::VariableDeclaration(TypedVariableDeclaration {
                name: parameter.name.clone(),
                body: TypedExpression {
                    expression: TypedExpressionVariant::FunctionParameter,
                    return_type: type_id,
                    is_constant: IsConstant::No,
                    span: parameter.name.span(),
                },
                is_mutable: if parameter.is_mutable {
                    VariableMutability::Mutable
                } else {
                    VariableMutability::Immutable
                },
                type_ascription: type_id,
            }),
        );
        let parameter = TypedFunctionParameter {
            name: parameter.name,
            is_mutable: parameter.is_mutable,
            type_id,
            type_span: parameter.type_span,
        };
        ok(parameter, warnings, errors)
    }
}
