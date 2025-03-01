use sway_types::{Span, Spanned};

use crate::type_engine::{is_type_info_storage_only, resolve_type, TypeId};
use crate::{error::ok, semantic_analysis, CompileError, CompileResult, CompileWarning};
use crate::{TypedDeclaration, TypedFunctionDeclaration};

use crate::semantic_analysis::{
    TypedAbiDeclaration, TypedAstNodeContent, TypedExpression, TypedExpressionVariant,
    TypedIntrinsicFunctionKind, TypedReturnStatement, TypedWhileLoop,
};

use super::{
    TypedEnumDeclaration, TypedImplTrait, TypedStorageDeclaration, TypedStructDeclaration,
    TypedTraitDeclaration,
};

fn ast_node_validate(x: &TypedAstNodeContent) -> CompileResult<()> {
    let mut errors: Vec<CompileError> = vec![];
    let mut warnings: Vec<CompileWarning> = vec![];
    match x {
        TypedAstNodeContent::ReturnStatement(TypedReturnStatement { expr })
        | TypedAstNodeContent::Expression(expr)
        | TypedAstNodeContent::ImplicitReturnExpression(expr) => expr_validate(expr),
        TypedAstNodeContent::Declaration(decl) => decl_validate(decl),
        TypedAstNodeContent::WhileLoop(TypedWhileLoop { condition, body }) => {
            check!(expr_validate(condition), (), warnings, errors);
            check!(
                validate_decls_for_storage_only_types_in_codeblock(body),
                (),
                warnings,
                errors
            );
            ok((), warnings, errors)
        }
        TypedAstNodeContent::SideEffect => ok((), warnings, errors),
    }
}

fn expr_validate(expr: &TypedExpression) -> CompileResult<()> {
    let mut errors: Vec<CompileError> = vec![];
    let mut warnings: Vec<CompileWarning> = vec![];
    match &expr.expression {
        TypedExpressionVariant::Literal(_)
        | TypedExpressionVariant::VariableExpression { .. }
        | TypedExpressionVariant::FunctionParameter
        | TypedExpressionVariant::AsmExpression { .. }
        | TypedExpressionVariant::StorageAccess(_)
        | TypedExpressionVariant::AbiName(_) => (),
        TypedExpressionVariant::FunctionApplication { arguments, .. } => {
            for f in arguments {
                check!(expr_validate(&f.1), continue, warnings, errors);
            }
        }
        TypedExpressionVariant::LazyOperator {
            lhs: expr1,
            rhs: expr2,
            ..
        }
        | TypedExpressionVariant::ArrayIndex {
            prefix: expr1,
            index: expr2,
        } => {
            check!(expr_validate(&*expr1), (), warnings, errors);
            check!(expr_validate(&*expr2), (), warnings, errors);
        }
        TypedExpressionVariant::IntrinsicFunction(TypedIntrinsicFunctionKind {
            arguments: exprvec,
            ..
        })
        | TypedExpressionVariant::Tuple { fields: exprvec }
        | TypedExpressionVariant::Array { contents: exprvec } => {
            for f in exprvec {
                check!(expr_validate(f), continue, warnings, errors)
            }
        }
        TypedExpressionVariant::StructExpression { fields, .. } => {
            for f in fields {
                check!(expr_validate(&f.value), continue, warnings, errors);
            }
        }
        TypedExpressionVariant::CodeBlock(cb) => {
            check!(
                validate_decls_for_storage_only_types_in_codeblock(cb),
                (),
                warnings,
                errors
            );
        }
        TypedExpressionVariant::IfExp {
            condition,
            then,
            r#else,
        } => {
            check!(expr_validate(&*condition), (), warnings, errors);
            check!(expr_validate(&*then), (), warnings, errors);
            if let Some(r#else) = r#else {
                check!(expr_validate(&*r#else), (), warnings, errors);
            }
        }
        TypedExpressionVariant::StructFieldAccess { prefix: exp, .. }
        | TypedExpressionVariant::TupleElemAccess { prefix: exp, .. }
        | TypedExpressionVariant::AbiCast { address: exp, .. }
        | TypedExpressionVariant::EnumTag { exp }
        | TypedExpressionVariant::UnsafeDowncast { exp, .. } => {
            check!(expr_validate(&*exp), (), warnings, errors)
        }
        TypedExpressionVariant::EnumInstantiation { contents, .. } => {
            if let Some(f) = contents {
                check!(expr_validate(&*f), (), warnings, errors);
            }
        }
    }
    ok((), warnings, errors)
}

fn check_type(ty: TypeId, span: Span, ignore_self: bool) -> CompileResult<()> {
    let mut warnings: Vec<CompileWarning> = vec![];
    let mut errors: Vec<CompileError> = vec![];

    let ti = resolve_type(ty, &span).unwrap();
    let nested_types = check!(
        ti.clone().extract_nested_types(&span),
        vec![],
        warnings,
        errors
    );
    for ty in nested_types {
        if ignore_self && ty == ti {
            continue;
        }
        if is_type_info_storage_only(&ty) {
            errors.push(CompileError::InvalidStorageOnlyTypeDecl {
                ty: ty.to_string(),
                span: span.clone(),
            });
        }
    }
    ok((), warnings, errors)
}

fn decl_validate(decl: &TypedDeclaration) -> CompileResult<()> {
    let mut warnings: Vec<CompileWarning> = vec![];
    let mut errors: Vec<CompileError> = vec![];
    match decl {
        TypedDeclaration::VariableDeclaration(semantic_analysis::TypedVariableDeclaration {
            body: expr,
            name,
            ..
        })
        | TypedDeclaration::ConstantDeclaration(semantic_analysis::TypedConstantDeclaration {
            value: expr,
            name,
            ..
        })
        | TypedDeclaration::Reassignment(semantic_analysis::TypedReassignment {
            rhs: expr,
            lhs_base_name: name,
            ..
        }) => {
            check!(
                check_type(expr.return_type, name.span(), false),
                (),
                warnings,
                errors
            );
            check!(expr_validate(expr), (), warnings, errors)
        }
        TypedDeclaration::FunctionDeclaration(TypedFunctionDeclaration {
            body,
            parameters,
            ..
        }) => {
            check!(
                validate_decls_for_storage_only_types_in_codeblock(body),
                (),
                warnings,
                errors
            );
            for param in parameters {
                check!(
                    check_type(param.type_id, param.type_span.clone(), false),
                    continue,
                    warnings,
                    errors
                );
            }
        }
        TypedDeclaration::AbiDeclaration(TypedAbiDeclaration { methods: _, .. })
        | TypedDeclaration::TraitDeclaration(TypedTraitDeclaration { methods: _, .. }) => {
            // These methods are not typed. They are however handled from ImplTrait.
        }
        TypedDeclaration::ImplTrait(TypedImplTrait { methods, .. }) => {
            for method in methods {
                check!(
                    validate_decls_for_storage_only_types_in_codeblock(&method.body),
                    continue,
                    warnings,
                    errors
                )
            }
        }
        TypedDeclaration::StructDeclaration(TypedStructDeclaration { fields, .. }) => {
            for field in fields {
                check!(
                    check_type(field.type_id, field.span.clone(), false),
                    continue,
                    warnings,
                    errors
                );
            }
        }
        TypedDeclaration::EnumDeclaration(TypedEnumDeclaration { variants, .. }) => {
            for variant in variants {
                check!(
                    check_type(variant.type_id, variant.span.clone(), false),
                    continue,
                    warnings,
                    errors
                );
            }
        }
        TypedDeclaration::StorageDeclaration(TypedStorageDeclaration { fields, .. }) => {
            for field in fields {
                check!(
                    check_type(field.type_id, field.name.span().clone(), true),
                    continue,
                    warnings,
                    errors
                );
            }
        }
        TypedDeclaration::GenericTypeForFunctionScope { .. }
        | TypedDeclaration::ErrorRecovery
        | TypedDeclaration::StorageReassignment(_)
        | TypedDeclaration::Break { .. }
        | TypedDeclaration::Continue { .. } => {}
    }
    ok((), warnings, errors)
}

pub fn validate_decls_for_storage_only_types_in_ast(
    ast_n: &TypedAstNodeContent,
) -> CompileResult<()> {
    ast_node_validate(ast_n)
}

pub fn validate_decls_for_storage_only_types_in_codeblock(
    cb: &semantic_analysis::TypedCodeBlock,
) -> CompileResult<()> {
    let mut warnings: Vec<CompileWarning> = vec![];
    let mut errors: Vec<CompileError> = vec![];
    for x in &cb.contents {
        check!(ast_node_validate(&x.content), continue, warnings, errors)
    }
    ok((), warnings, errors)
}
