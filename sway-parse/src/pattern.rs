use crate::{Parse, ParseBracket, ParseErrorKind, ParseResult, Parser};

use sway_ast::brackets::{Braces, Parens};
use sway_ast::keywords::{DoubleDotToken, FalseToken, TrueToken};
use sway_ast::literal::{LitBool, LitBoolType};
use sway_ast::punctuated::Punctuated;
use sway_ast::{Literal, PathExpr, Pattern, PatternStructField};
use sway_types::Spanned;

impl Parse for Pattern {
    fn parse(parser: &mut Parser) -> ParseResult<Pattern> {
        if let Some(mut_token) = parser.take() {
            let mutable = Some(mut_token);
            let name = parser.parse()?;
            return Ok(Pattern::Var { mutable, name });
        }
        if parser.peek::<TrueToken>().is_some() {
            let ident = parser.parse::<TrueToken>()?;
            return Ok(Pattern::Literal(Literal::Bool(LitBool {
                span: ident.span(),
                kind: LitBoolType::True,
            })));
        }
        if parser.peek::<FalseToken>().is_some() {
            let ident = parser.parse::<FalseToken>()?;
            return Ok(Pattern::Literal(Literal::Bool(LitBool {
                span: ident.span(),
                kind: LitBoolType::False,
            })));
        }
        if let Some(literal) = parser.take() {
            return Ok(Pattern::Literal(literal));
        }
        if let Some(tuple) = Parens::try_parse(parser)? {
            return Ok(Pattern::Tuple(tuple));
        }
        if let Some(underscore_token) = parser.take() {
            return Ok(Pattern::Wildcard { underscore_token });
        }

        let path = parser.parse::<PathExpr>()?;
        if let Some(args) = Parens::try_parse(parser)? {
            return Ok(Pattern::Constructor { path, args });
        }
        if let Some(fields) = Braces::try_parse(parser)? {
            let inner_fields: &Punctuated<_, _> = fields.get();
            let rest_pattern = inner_fields
                .value_separator_pairs
                .iter()
                .find(|(p, _)| matches!(p, PatternStructField::Rest { token: _ }));

            if let Some((rest_pattern, _)) = rest_pattern {
                return Err(parser.emit_error_with_span(
                    ParseErrorKind::UnexpectedRestPattern,
                    rest_pattern.span(),
                ));
            }

            return Ok(Pattern::Struct { path, fields });
        }
        match path.try_into_ident() {
            Ok(name) => Ok(Pattern::Var {
                mutable: None,
                name,
            }),
            Err(path) => Ok(Pattern::Constant(path)),
        }
    }
}

impl Parse for PatternStructField {
    fn parse(parser: &mut Parser) -> ParseResult<PatternStructField> {
        if parser.peek::<DoubleDotToken>().is_some() {
            let token = parser.parse()?;
            return Ok(PatternStructField::Rest { token });
        }

        let field_name = parser.parse()?;
        let pattern_opt = match parser.take() {
            Some(colon_token) => {
                let pattern = parser.parse()?;
                Some((colon_token, pattern))
            }
            None => None,
        };
        Ok(PatternStructField::Field {
            field_name,
            pattern_opt,
        })
    }
}
