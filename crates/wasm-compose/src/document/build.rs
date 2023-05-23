use super::{error, parse, token::Tokenizer};
use crate::{composer::CompositionGraphBuilder, config::Config};

use anyhow::{anyhow, Context, Result};
use std::{fs, path::Path};

pub fn build_graph<'a>(config: &'a Config, path: &'a Path) -> Result<CompositionGraphBuilder<'a>> {
    let mut graph_builder = CompositionGraphBuilder::new(config);

    let input =
        fs::read_to_string(path).with_context(|| anyhow!("failed to read document at {path:?}"))?;

    let mut tokens = Tokenizer::new(&input, 0)?;
    let document = parse::Ast::parse(&mut tokens)?;

    // let imports = document.resolve_imports(&mut graph_builder)?;

    todo!("build-graph");

    Ok(graph_builder)
}
