//! Module for WebAssembly composition documents.

use anyhow::Result;
use std::path::Path;

pub use self::parse::Ast;

mod error;
mod parse;
mod token;

/// Parse an composition document into its AST.
pub fn parse<'i>(path: impl AsRef<Path>, source: &'i str) -> Result<Ast<'i>> {
    let mut tokens = token::Tokenizer::new(&source, 0)?;

    let path = path.as_ref();

    let ast = match Ast::parse(&mut tokens) {
        Ok(ast) => ast,
        Err(mut err) => {
            let file = path.display().to_string();
            error::rewrite(&mut err, &file, &source);
            return Err(err);
        }
    };

    Ok(ast)
}