use super::{
    error,
    token::{Span, Token, Tokenizer},
};
use anyhow::Result;
use semver::Version;
use std::{borrow::Cow, fmt};

/// Composition document AST.
#[derive(Debug)]
pub struct Ast<'i> {
    pub items: Vec<Item<'i>>,
}

impl<'i> Ast<'i> {
    pub fn parse(tokens: &mut Tokenizer<'i>) -> Result<Ast<'i>> {
        let mut items = Vec::new();
        while tokens.clone().next()?.is_some() {
            let docs = Docs::parse(tokens)?;
            items.push(Item::parse(tokens, docs)?);
        }
        Ok(Self { items })
    }

    pub fn for_each_import<'b>(
        &'b self,
        mut f: impl FnMut(&'b Name<'i>, &'b ImportKind<'i>) -> Result<()>,
    ) -> Result<()> {
        for item in self.items.iter() {
            match item {
                Item::Import(Import { name, kind, .. }) => {
                    f(name, kind)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub fn for_each_export<'b>(
        &'b self,
        mut f: impl FnMut(&'b Expr<'i>, Option<&'b Id<'i>>) -> Result<()>,
    ) -> Result<()> {
        for item in self.items.iter() {
            match item {
                Item::Export(Export { expr, as_, .. }) => {
                    f(expr, as_.as_ref())?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub fn for_each_instantiation<'b>(
        &'b self,
        mut f: impl FnMut(&'b Name<'i>, &'b Expr<'i>) -> Result<()>,
    ) -> Result<()> {
        for item in self.items.iter() {
            match item {
                Item::Let(Let { var, expr, .. }) => {
                    f(var, expr)?;
                }
                _ => {}
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum Item<'i> {
    Import(Import<'i>),
    Export(Export<'i>),
    Let(Let<'i>),
}

impl<'i> Item<'i> {
    pub fn parse(tokens: &mut Tokenizer<'i>, docs: Docs<'i>) -> Result<Item<'i>> {
        match tokens.clone().next()? {
            Some((_span, Token::Component)) => {
                Import::parse_component(tokens, docs).map(Item::Import)
            }
            Some((_span, Token::Import)) => Import::parse_interface(tokens, docs).map(Item::Import),
            Some((_span, Token::Export)) => Export::parse(tokens, docs).map(Item::Export),
            Some((_span, Token::Let)) => Let::parse(tokens, docs).map(Item::Let),
            other => {
                Err(
                    error::expected(tokens, "`component`, `import`, `export` or `let`", other)
                        .into(),
                )
            }
        }
    }
}

pub struct Import<'i> {
    pub docs: Docs<'i>,
    pub name: Name<'i>,
    pub kind: ImportKind<'i>,
}

impl fmt::Debug for Import<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Import")
            .field("name", &self.name)
            .field("kind", &self.kind)
            .finish()
    }
}

#[derive(Debug)]
pub enum ImportKind<'i> {
    Component(Option<Id<'i>>),
    Interface(Id<'i>),
}

impl<'i> Import<'i> {
    fn parse_component(tokens: &mut Tokenizer<'i>, docs: Docs<'i>) -> Result<Import<'i>> {
        tokens.expect(Token::Component)?;
        let name = Name::parse(tokens)?;
        let mut id = None;
        if tokens.eat(Token::Colon)? {
            id = Id::parse(tokens).map(Option::Some)?
        }
        Ok(Import {
            docs,
            name,
            kind: ImportKind::Component(id),
        })
    }

    fn parse_interface(tokens: &mut Tokenizer<'i>, docs: Docs<'i>) -> Result<Import<'i>> {
        tokens.expect(Token::Import)?;
        let name = Name::parse(tokens)?;
        tokens.expect(Token::Colon)?;
        let id = Id::parse(tokens)?;
        Ok(Import {
            docs,
            name,
            kind: ImportKind::Interface(id),
        })
    }
}

pub struct Export<'i> {
    pub docs: Docs<'i>,
    pub expr: Expr<'i>,
    pub as_: Option<Id<'i>>,
}

impl fmt::Debug for Export<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Export")
            .field("expr", &self.expr)
            .field("as", &self.as_)
            .finish()
    }
}

impl<'i> Export<'i> {
    fn parse(tokens: &mut Tokenizer<'i>, docs: Docs<'i>) -> Result<Export<'i>> {
        tokens.expect(Token::Export)?;
        let expr = Expr::parse(tokens)?;
        let mut as_ = None;
        if tokens.eat(Token::As)? {
            as_ = Id::parse(tokens).map(Option::Some)?;
        }
        Ok(Export { docs, expr, as_ })
    }
}

pub struct Let<'i> {
    pub docs: Docs<'i>,
    pub var: Name<'i>,
    pub expr: Expr<'i>,
}

impl fmt::Debug for Let<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Let")
            .field("var", &self.var)
            .field("expr", &self.expr)
            .finish()
    }
}

impl<'i> Let<'i> {
    fn parse(tokens: &mut Tokenizer<'i>, docs: Docs<'i>) -> Result<Let<'i>> {
        tokens.expect(Token::Let)?;
        let var = Name::parse(tokens)?;
        tokens.expect(Token::Equals)?;
        tokens.expect(Token::New)?;
        let expr = Expr::parse(tokens)?;
        Ok(Let { docs, var, expr })
    }
}

// let foobar = new local:foobar/
#[derive(Debug)]
pub struct Expr<'i> {
    pub name: Name<'i>,
    pub args: Option<Args<'i>>,
}

impl<'i> Expr<'i> {
    pub fn parse(tokens: &mut Tokenizer<'i>) -> Result<Expr<'i>> {
        let name = Name::parse(tokens)?;
        let args = if tokens.eat(Token::LeftBrace)? {
            Args::parse(tokens).map(Option::Some)?
        } else {
            None
        };
        Ok(Expr { name, args })
    }
}

#[derive(Debug)]
pub struct Args<'i>(pub Vec<Arg<'i>>);

impl<'i> Args<'i> {
    fn parse(tokens: &mut Tokenizer<'i>) -> Result<Args<'i>> {
        parse_list_trailer(tokens, Token::RightBrace, Arg::parse).map(Args)
    }
}

pub struct Arg<'i> {
    pub docs: Docs<'i>,
    pub name: Name<'i>,
    pub with: Option<Name<'i>>,
}

impl fmt::Debug for Arg<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.name)
    }
}

impl<'i> Arg<'i> {
    fn parse(tokens: &mut Tokenizer<'i>, docs: Docs<'i>) -> Result<Arg<'i>> {
        let name = Name::parse(tokens)?;

        let with = match tokens.clone().next()? {
            Some((_span, Token::Comma | Token::RightBrace)) => None,
            Some((_span, Token::Colon)) => {
                tokens.expect(Token::Colon)?;
                Name::parse(tokens).map(Option::Some)?
            }
            other => {
                return Err(error::expected(tokens, "argument", other).into());
            }
        };

        Ok(Arg { docs, name, with })
    }
}

// e.g. foo:bar/baz@1.0
pub struct Id<'i> {
    pub namespace: Name<'i>,
    pub package: Name<'i>,
    pub element: Name<'i>,
    pub version: Option<(Span, Version)>,
}

impl fmt::Debug for Id<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let Id {
            namespace,
            package,
            element,
            version: _,
        } = self;

        write!(
            f,
            "{namespace}:{package}/{element}",
            namespace = namespace.name,
            package = package.name,
            element = element.name,
        )?;
        // if let Some((_, major, minor)) = version {
        //     write!(f, "@{major}.{minor}")?;
        // }
        Ok(())
    }
}

impl<'i> Id<'i> {
    fn parse(tokens: &mut Tokenizer<'i>) -> Result<Id<'i>> {
        let namespace = Name::parse(tokens)?;
        tokens.expect(Token::Colon)?;
        let package = Name::parse(tokens)?;
        tokens.expect(Token::Slash)?;
        let element = Name::parse(tokens)?;
        let version = None; //parse_opt_version(tokens)?;
        Ok(Id {
            namespace,
            package,
            element,
            version,
        })
    }

    pub fn as_wit_package_name(&self) -> wit_parser::PackageName {
        wit_parser::PackageName {
            namespace: self.namespace.name.to_string(),
            name: self.package.name.to_string(),
            version: None, //self.version.map(|(_, major, minor)| (major, minor)),
        }
    }
}

#[derive(Clone)]
pub struct Name<'i> {
    pub name: &'i str,
    pub span: Span,
}

impl<'i> From<&'i str> for Name<'i> {
    fn from(s: &'i str) -> Name<'i> {
        Name {
            name: s.into(),
            span: Span { start: 0, end: 0 },
        }
    }
}

impl fmt::Debug for Name<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(&format!(
            "{name}@{pos}..{end}",
            name = self.name,
            pos = self.span.start,
            end = self.span.end,
        ))
        .finish()
    }
}

impl<'i> Name<'i> {
    fn parse(tokens: &mut Tokenizer<'i>) -> Result<Name<'i>> {
        match tokens.next()? {
            Some((span, Token::Id)) => Ok(Name {
                name: tokens.parse_id(span)?,
                span,
            }),
            other => {
                let error = error::expected(tokens, "a kebab-case name", other);
                Err(error.into())
            }
        }
    }
}

#[derive(Default)]
pub struct Docs<'i> {
    docs: Vec<Cow<'i, str>>,
}

impl<'i> Docs<'i> {
    fn parse(tokens: &mut Tokenizer<'i>) -> Result<Docs<'i>> {
        let mut docs = Docs::default();
        let mut clone = tokens.clone();
        while let Some((span, token)) = clone.next_raw()? {
            match token {
                Token::Whitespace => {}
                Token::Comment => docs.docs.push(tokens.get_span(span).into()),
                _ => break,
            };
            *tokens = clone.clone();
        }
        Ok(docs)
    }
}

// fn parse_opt_version(tokens: &mut Tokenizer<'_>) -> Result<Option<(Span, u32, u32)>> {
//     Ok(if tokens.eat(Token::At)? {
//         let major = tokens.expect(Token::Integer)?;
//         tokens.expect(Token::Period)?;
//         let minor = tokens.expect(Token::Integer)?;
//         let span = Span {
//             start: major.start,
//             end: minor.end,
//         };
//         Some((span, tokens.parse_u32(major)?, tokens.parse_u32(minor)?))
//     } else {
//         None
//     })
// }

fn parse_list_trailer<'a, T>(
    tokens: &mut Tokenizer<'a>,
    end: Token,
    mut parse: impl FnMut(&mut Tokenizer<'a>, Docs<'a>) -> Result<T>,
) -> Result<Vec<T>> {
    let mut items = Vec::new();
    loop {
        // get docs before we skip them to try to eat the end token
        let docs = Docs::parse(tokens)?;

        // if we found an end token then we're done
        if tokens.eat(end)? {
            break;
        }

        let item = parse(tokens, docs)?;
        items.push(item);

        // if there's no trailing comma then this is required to be the end,
        // otherwise we go through the loop to try to get another item
        if !tokens.eat(Token::Comma)? {
            tokens.expect(end)?;
            break;
        }
    }
    Ok(items)
}
