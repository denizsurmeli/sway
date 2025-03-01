use crate::{Parse, ParseBracket, ParseErrorKind, ParseResult, Parser};

use sway_ast::attribute::Annotated;
use sway_ast::keywords::Keyword;
use sway_ast::{Braces, FnSignature, ItemFn, ItemTrait, Traits};
use sway_types::Spanned;

impl Parse for ItemTrait {
    fn parse(parser: &mut Parser) -> ParseResult<ItemTrait> {
        let visibility = parser.take();
        let trait_token = parser.parse()?;
        let name = parser.parse()?;
        let super_traits = match parser.take() {
            Some(colon_token) => {
                let traits = parser.parse()?;
                Some((colon_token, traits))
            }
            None => None,
        };

        let trait_items: Braces<Vec<(Annotated<FnSignature>, _)>> = parser.parse()?;
        for item in trait_items.get().iter() {
            let (fn_sig, _) = item;
            if let Some(token) = &fn_sig.value.visibility {
                return Err(parser.emit_error_with_span(
                    ParseErrorKind::UnnecessaryVisibilityQualifier {
                        visibility: token.ident(),
                    },
                    token.span(),
                ));
            }
        }

        let trait_defs_opt: Option<Braces<Vec<Annotated<ItemFn>>>> = Braces::try_parse(parser)?;
        if let Some(trait_defs) = &trait_defs_opt {
            for item in trait_defs.get().iter() {
                if let Some(token) = &item.value.fn_signature.visibility {
                    return Err(parser.emit_error_with_span(
                        ParseErrorKind::UnnecessaryVisibilityQualifier {
                            visibility: token.ident(),
                        },
                        token.span(),
                    ));
                }
            }
        }

        Ok(ItemTrait {
            visibility,
            trait_token,
            name,
            super_traits,
            trait_items,
            trait_defs_opt,
        })
    }
}

impl Parse for Traits {
    fn parse(parser: &mut Parser) -> ParseResult<Traits> {
        let prefix = parser.parse()?;
        let mut suffixes = Vec::new();
        while let Some(add_token) = parser.take() {
            let suffix = parser.parse()?;
            suffixes.push((add_token, suffix));
        }
        let traits = Traits { prefix, suffixes };
        Ok(traits)
    }
}
