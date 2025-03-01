use crate::{
    fmt::*,
    utils::comments::{ByteSpan, LeafSpans},
};
use std::fmt::Write;
use sway_ast::{Statement, StatementLet};
use sway_types::Spanned;

impl Format for Statement {
    fn format(
        &self,
        formatted_code: &mut FormattedCode,
        formatter: &mut Formatter,
    ) -> Result<(), FormatterError> {
        match self {
            Self::Let(let_stmt) => let_stmt.format(formatted_code, formatter)?,
            Self::Item(item) => item.format(formatted_code, formatter)?,
            Self::Expr {
                expr,
                semicolon_token_opt,
            } => {
                expr.format(formatted_code, formatter)?;
                if let Some(semicolon) = semicolon_token_opt {
                    write!(formatted_code, "{}", semicolon.span().as_str())?;
                }
            }
        }
        Ok(())
    }
}

impl Format for StatementLet {
    fn format(
        &self,
        formatted_code: &mut FormattedCode,
        formatter: &mut Formatter,
    ) -> Result<(), FormatterError> {
        // Add indent level + `let `
        write!(
            formatted_code,
            "{}{} ",
            formatter.shape.indent.to_string(&formatter.config)?,
            self.let_token.span().as_str()
        )?;
        // pattern
        self.pattern.format(formatted_code, formatter)?;
        // `: Ty`
        if let Some(ty) = &self.ty_opt {
            write!(formatted_code, "{} ", ty.0.span().as_str())?;
            ty.1.format(formatted_code, formatter)?;
        }
        // ` = `
        write!(formatted_code, " {} ", self.eq_token.span().as_str())?;
        // expr
        self.expr.format(formatted_code, formatter)?;
        // `;\n`
        writeln!(formatted_code, "{}", self.semicolon_token.span().as_str())?;

        Ok(())
    }
}

impl LeafSpans for Statement {
    fn leaf_spans(&self) -> Vec<ByteSpan> {
        match self {
            Statement::Let(statement_let) => statement_let.leaf_spans(),
            Statement::Item(item) => item.leaf_spans(),
            Statement::Expr {
                expr,
                semicolon_token_opt,
            } => {
                let mut collected_spans = expr.leaf_spans();
                if let Some(semicolon_token) = semicolon_token_opt {
                    collected_spans.push(ByteSpan::from(semicolon_token.span()));
                }
                collected_spans
            }
        }
    }
}

impl LeafSpans for StatementLet {
    fn leaf_spans(&self) -> Vec<ByteSpan> {
        // Add let token's ByteSpan
        let mut collected_spans = vec![ByteSpan::from(self.let_token.span())];
        // Add pattern's ByteSpan
        collected_spans.append(&mut self.pattern.leaf_spans());
        // Add ty's ByteSpan if it exists
        if let Some(ty) = &self.ty_opt {
            collected_spans.push(ByteSpan::from(ty.0.span()));
            collected_spans.append(&mut ty.1.leaf_spans());
        }
        // Add eq token's ByteSpan
        collected_spans.push(ByteSpan::from(self.eq_token.span()));
        // Add Expr's ByteSpan
        collected_spans.append(&mut self.expr.leaf_spans());
        collected_spans.push(ByteSpan::from(self.semicolon_token.span()));
        collected_spans
    }
}
