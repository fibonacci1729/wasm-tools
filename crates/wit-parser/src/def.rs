struct ComponentDef<'a> {
    docs: Docs<'a>,
    name: Id<'a>,
    items: Vec<ComponentItem<'a>>,
}

impl<'a> ComponentDef<'a> {
    fn parse(tokens: &mut Tokenizer<'a>, docs: Docs<'a>) -> Result<Self> {
        tokens.expect(Token::Component)?;
        let name = parse_id(tokens)?;
        let items = Self::parse_items(tokens)?;
        Ok(ComponentDef { docs, name, items })
    }

    fn parse_items(tokens: &mut Tokenizer<'a>) -> Result<Vec<ComponentItem<'a>>> {
        tokens.expect(Token::LeftBrace)?;
        let mut items = Vec::new();
        loop {
            let docs = parse_docs(tokens)?;
            if tokens.eat(Token::RightBrace)? {
                break;
            }
            items.push(ComponentItem::parse(tokens, docs)?);
        }
        Ok(items)
    }
}

enum ComponentItem<'a> {
    Import(ComponentImport<'a>),
    Let(ComponentLetStmt<'a>),
    Use {
        docs: Docs<'a>,
        package: PackageName<'a>,
        as_: Id<'a>,      
    }
}

enum ComponentImportKind<'a> {
    Interface(Vec<InterfaceItem<'a>>),
    Path(UsePath<'a>),
    Func(Func<'a>),
}

impl<'a> ComponentImportKind<'a> {
    fn parse(tokens: &mut Tokenizer<'a>) -> Result<ComponentImportKind<'a>> {
        match tokens.clone().next()? {
            Some((_span, Token::Interface)) => Interface::parse_items(tokens).map(ComponentImportKind::Interface),
            Some((_span, Token::Func)) => Func::parse(tokens).map(ComponentImportKind::Func),
            Some((_span, Token::Id)) => UsePath::parse(tokens).map(ComponentImportKind::Path),
            other => Err(err_expected(tokens, "`func`, `interface` or `path`", other).into()),
        }
    }
}

struct ComponentImport<'a> {
    docs: Docs<'a>,
    name: Id<'a>,
    kind: ComponentImportKind<'a>,
}

impl<'a> ComponentImport<'a> {
    fn parse(tokens: &mut Tokenizer<'a>, docs: Docs<'a>) -> Result<ComponentImport<'a>> {
        tokens.expect(Token::Import)?;
        let name = parse_id(tokens)?;
        tokens.expect(Token::Colon)?;
        let kind = ComponentImportKind::parse(tokens)?;
        Ok(ComponentImport { docs, name, kind })
    }
}

impl<'a> ComponentItem<'a> {
    fn parse(tokens: &mut Tokenizer<'a>, docs: Docs<'a>) -> Result<ComponentItem<'a>> {
        match tokens.clone().next()? {
            Some((_span, Token::Import)) => ComponentImport::parse(tokens, docs).map(ComponentItem::Import),
            Some((_span, Token::Let)) => ComponentLetStmt::parse(tokens, docs).map(ComponentItem::Let),
            Some((_span, Token::Use)) => Self::parse_use(tokens, docs),
            other => Err(err_expected(
                tokens,
                "`import`, `export`, `use`, or `let`",
                other,
            )
            .into()),
        }
    }

    fn parse_use(tokens: &mut Tokenizer<'a>, docs: Docs<'a>) -> Result<ComponentItem<'a>> {
        tokens.expect(Token::Use)?;
        let package = PackageName::parse(tokens)?;
        tokens.expect(Token::As)?;
        let as_ = parse_id(tokens)?;
        Ok(ComponentItem::Use { docs, package, as_ })
    }
}

struct PathExpr<'a> {
    from: Id<'a>,
    elems: Vec<UsePath<'a>>,
}

impl<'a> PathExpr<'a> {
    fn parse(tokens: &mut Tokenizer<'a>) -> Result<PathExpr<'a>> {
        let from = parse_id(tokens)?;
        let mut elems = Vec::new();
        loop {
            if !tokens.eat(Token::LeftBracket)? {
                break
            }
            elems.push(UsePath::parse(tokens)?);
            tokens.expect(Token::RightBrace)?;
        }
        Ok(PathExpr { from, elems })
    }
}

struct ComponentLetStmt<'a> {
    docs: Docs<'a>,
    name: Id<'a>,
    expr: Expr<'a>,
}

impl<'a> ComponentLetStmt<'a> {
    fn parse(tokens: &mut Tokenizer<'a>, docs: Docs<'a>) -> Result<ComponentLetStmt<'a>> {
        tokens.expect(Token::Let)?;
        let name = parse_id(tokens)?;
        tokens.expect(Token::Equals)?;
        let expr = Expr::parse(tokens)?;
        Ok(ComponentLetStmt { docs, name, expr })
    }
}

enum Expr<'a> {
    New(InstantiateExpr<'a>),
    Alias(PathExpr<'a>),
}

impl<'a> Expr<'a> {
    fn parse(tokens: &mut Tokenizer<'a>) -> Result<Expr<'a>> {
        match tokens.clone().next()? {
            Some((_span, Token::New)) => InstantiateExpr::parse(tokens).map(Expr::New),
            Some((_span, Token::Id)) => PathExpr::parse(tokens).map(Expr::Alias),
            other => Err(err_expected(
                tokens,
                "`new` expression", //"`new` or path expression",
                other,
            )
            .into()),
        }
    }
}

struct InstantiateExpr<'a> {
    name: Id<'a>,
    args: Vec<InstantiateArg<'a>>,
}

impl<'a> InstantiateExpr<'a> {
    fn parse(tokens: &mut Tokenizer<'a>) -> Result<InstantiateExpr<'a>> {
        tokens.expect(Token::New)?;
        let name = parse_id(tokens)?;
        let args = if tokens.eat(Token::LeftBrace)? {
            parse_list_trailer(tokens, Token::RightBrace, InstantiateArg::parse)?
        } else {
            Vec::new()
        };
        Ok(InstantiateExpr { name, args })
    }
}

struct InstantiateArg<'a> {
    docs: Docs<'a>,
    name: Option<Id<'a>>,
    expr: Expr<'a>,
}

impl<'a> InstantiateArg<'a> {
    fn parse(docs: Docs<'a>, tokens: &mut Tokenizer<'a>) -> Result<InstantiateArg<'a>> {
        let mut clone = tokens.clone();
        match clone.next()? {
            Some((_span, Token::Id)) => {
                if clone.eat(Token::Equals)? { 
                    // name `=` expr
                    let name = parse_id(tokens).map(Option::Some)?;
                    tokens.expect(Token::Equals)?;
                    let expr = Expr::parse(tokens)?;
                    Ok(InstantiateArg {
                        docs,
                        name,
                        expr,
                    })
                } else {
                    // ... otherwise must be path expression
                    Ok(InstantiateArg {
                        docs,
                        name: None,
                        expr: PathExpr::parse(tokens).map(Expr::Alias)?,
                    })
                }
            }
            Some((_span, Token::New)) => {
                Ok(InstantiateArg {
                    docs,
                    name: None,
                    expr: InstantiateExpr::parse(tokens).map(Expr::New)?,
                })
            }
            other => Err(err_expected(
                tokens,
                "argument name or expression",
                other,
            )
            .into()),
        }
    }  
}
