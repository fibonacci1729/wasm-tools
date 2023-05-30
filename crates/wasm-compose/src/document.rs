use crate::{composer::CompositionGraphBuilder, graph::{Component, InstanceId, ComponentId, CompositionGraph}};
use anyhow::{Context, Result};
use std::{fs, path::Path};

use self::{
    parse::Ast,
    token::Tokenizer,
};

mod error;
mod parse;
mod token;



pub(crate) fn build_graph<'a>(mut builder: CompositionGraphBuilder<'a>, path: &'a Path) -> Result<(InstanceId, CompositionGraph<'a>)> {
    let source = fs::read_to_string(path).with_context(|| format!("failed to read: {}", path.display()))?;

    let parent = path.parent().unwrap();

    let filename = path
        .file_name()
        .context("wld path must end in a file name")?
        .to_str()
        .context("wld filename must be valid unicode")?
        // TODO: replace with `file_prefix` if/when that gets stabilized.
        .split(".")
        .next()
        .unwrap();

    let mut tokens = Tokenizer::new(&source, 0)?;

    let ast = match Ast::parse(&mut tokens) {
        Ok(ast) => ast,
        Err(mut err) => {
            let file = path.display().to_string();
            error::rewrite(&mut err, &file, &source);
            return Err(err);
        }
    };

    todo!("resolve document");

    builder.build()
}
