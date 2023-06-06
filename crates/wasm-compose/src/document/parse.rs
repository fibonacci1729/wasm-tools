use super::{
    error,
    token::{Span, Token, Tokenizer},
};
use anyhow::Result;
use semver::Version;
use std::borrow::Cow;

/// Composition document AST.
pub struct Ast<'i> {
    items: Vec<Item<'i>>,
}

impl<'i> Ast<'i> {
    /// Parse a composition document.
    pub(super) fn parse(tokens: &mut Tokenizer<'i>) -> Result<Ast<'i>> {
        let mut items = Vec::new();
        while tokens.clone().next()?.is_some() {
            let docs = Docs::parse(tokens)?;
            items.push(Item::parse(tokens, docs)?);
        }
        Ok(Self { items })
    }

    // fn for_each_import<'b>(
    //     &'b self,
    //     mut f: impl FnMut(&'b Import<'i>) -> Result<()>,
    // ) -> Result<()> {
    //     for item in self.items.iter() {
    //         match item {
    //             Item::Import(item) => {
    //                 f(item)?;
    //             }
    //             _ => {}
    //         }
    //     }
    //     Ok(())
    // }

    // fn for_each_export<'b>(
    //     &'b self,
    //     mut f: impl FnMut(&'b Export<'i>) -> Result<()>,
    // ) -> Result<()> {
    //     for item in self.items.iter() {
    //         match item {
    //             Item::Export(item) => {
    //                 f(item)?;
    //             }
    //             _ => {}
    //         }
    //     }
    //     Ok(())
    // }

    // fn for_each_use<'b>(
    //     &'b self,
    //     mut f: impl FnMut(&'b Use<'i>) -> Result<()>,
    // ) -> Result<()> {
    //     for item in self.items.iter() {
    //         match item {
    //             Item::Use(item) => {
    //                 f(item)?;
    //             }
    //             _ => {}
    //         }
    //     }
    //     Ok(())
    // }

    // fn for_each_let<'b>(
    //     &'b self,
    //     mut f: impl FnMut(&'b Let<'i>) -> Result<()>,
    // ) -> Result<()> {
    //     for item in self.items.iter() {
    //         match item {
    //             Item::Let(item) => {
    //                 f(item)?;
    //             }
    //             _ => {}
    //         }
    //     }
    //     Ok(())
    // }
}

enum Item<'i> {
    Import(Import<'i>),
    Export(Export<'i>),
    Use(Use<'i>),
    Let(Let<'i>),
}

impl<'i> Item<'i> {
    fn parse(tokens: &mut Tokenizer<'i>, docs: Docs<'i>) -> Result<Item<'i>> {
        match tokens.clone().next()? {
            Some((_span, Token::Import)) => Import::parse(tokens, docs).map(Item::Import),
            Some((_span, Token::Export)) => Export::parse(tokens, docs).map(Item::Export),
            Some((_span, Token::Use)) => Use::parse(tokens, docs).map(Item::Use),
            Some((_span, Token::Let)) => Let::parse(tokens, docs).map(Item::Let),
            other => {
                Err(
                    error::expected(tokens, "`import`, `export`, `use` or `let`", other)
                        .into(),
                )
            }
        }
    }
}

struct Import<'i> {
    docs: Docs<'i>,
    name: Name<'i>,
    id: Id<'i>,
}

impl<'i> Import<'i> {
    fn parse(tokens: &mut Tokenizer<'i>, docs: Docs<'i>) -> Result<Import<'i>> {
        tokens.expect(Token::Import)?;
        let name = Name::parse(tokens)?;
        tokens.expect(Token::Colon)?;
        let id = Id::parse(tokens)?;
        Ok(Import { docs, name, id })
    }
}

struct Use<'i> {
    docs: Docs<'i>,
    pkg: PackageName<'i>,
    as_: Option<Name<'i>>,
}

impl<'i> Use<'i> {
    fn parse(tokens: &mut Tokenizer<'i>, docs: Docs<'i>) -> Result<Use<'i>> {
        tokens.expect(Token::Use)?;
        let pkg = PackageName::parse(tokens)?;
        let mut as_ = None;
        if tokens.eat(Token::As)? {
            as_ = Name::parse(tokens).map(Option::Some)?;
        }
        Ok(Use { docs, pkg, as_ })
    }
}

struct Export<'i> {
    docs: Docs<'i>,
    path: Path<'i>,
    as_: Option<NameOrId<'i>>,
}

impl<'i> Export<'i> {
    fn parse(tokens: &mut Tokenizer<'i>, docs: Docs<'i>) -> Result<Export<'i>> {
        tokens.expect(Token::Export)?;
        let path = Path::parse(tokens)?;
        let mut as_ = None;
        if tokens.eat(Token::As)? {
            as_ = NameOrId::parse(tokens).map(Option::Some)?;
        }
        Ok(Export { docs, path, as_ })
    }
}

struct Let<'i> {
    docs: Docs<'i>,
    name: Name<'i>,
    expr: Expr<'i>,
}

impl<'i> Let<'i> {
    fn parse(tokens: &mut Tokenizer<'i>, docs: Docs<'i>) -> Result<Let<'i>> {
        tokens.expect(Token::Let)?;
        let name = Name::parse(tokens)?;
        tokens.expect(Token::Equals)?;
        let expr = Expr::parse(tokens)?;
        Ok(Let { docs, name, expr })
    }
}

enum Expr<'i> {
    Alias(Path<'i>),
    New {
        name: Name<'i>,
        args: Option<Args<'i>>,
    },
}

impl<'i> Expr<'i> {
    fn parse(tokens: &mut Tokenizer<'i>) -> Result<Expr<'i>> {
        if tokens.eat(Token::New)? {
            Expr::parse_new(tokens)
        } else {
            Expr::parse_alias(tokens)
        }
    }

    fn parse_new(tokens: &mut Tokenizer<'i>) -> Result<Expr<'i>> {
        let name = Name::parse(tokens)?;
        let args = if tokens.eat(Token::LeftBrace)? {
            Args::parse(tokens).map(Option::Some)?
        } else {
            None
        };
        Ok(Expr::New { name, args })
    }

    fn parse_alias(tokens: &mut Tokenizer<'i>) -> Result<Expr<'i>> {
        Path::parse(tokens).map(Expr::Alias)
    }
}

/// A path expression
struct Path<'i> {
    from: Name<'i>,
    elems: Vec<NameOrId<'i>>,
}

impl<'i> Path<'i> {
    fn parse(tokens: &mut Tokenizer<'i>) -> Result<Path<'i>> {
        let from = Name::parse(tokens)?;
        let mut elems = Vec::new();
        while tokens.eat(Token::LeftBracket)? {
            let elem = NameOrId::parse(tokens)?;
            tokens.expect(Token::RightBracket)?;
            elems.push(elem);
        }
        Ok(Path { from, elems })
    }
}

/// Instantiation arguments
struct Args<'i>(pub Vec<Arg<'i>>);

impl<'i> Args<'i> {
    fn parse(tokens: &mut Tokenizer<'i>) -> Result<Args<'i>> {
        parse_list_trailer(tokens, Token::RightBrace, Arg::parse).map(Args)
    }
}

/// An instantiation argument
struct Arg<'i> {
    docs: Docs<'i>,
    name: Option<Name<'i>>,
    expr: Expr<'i>,
}

impl<'i> Arg<'i> {
    fn parse(tokens: &mut Tokenizer<'i>, docs: Docs<'i>) -> Result<Arg<'i>> {
        let mut clone = tokens.clone();
        match clone.next()? {
            Some((_span, Token::Id)) => {
                if clone.eat(Token::Equals)? { 
                    // name `=` expr
                    let name = Name::parse(tokens).map(Option::Some)?;
                    tokens.expect(Token::Equals)?;
                    let expr = Expr::parse(tokens)?;
                    Ok(Arg {
                        docs,
                        name,
                        expr,
                    })
                } else {
                    // ... otherwise must be alias expression
                    Ok(Arg {
                        docs,
                        name: None,
                        expr: Expr::parse_alias(tokens)?,
                    })
                }
            }
            Some((_span, Token::New)) => {
                Ok(Arg {
                    docs,
                    name: None,
                    expr: Expr::parse_new(tokens)?,
                })
            }
            other => Err(error::expected(tokens, "argument name or expression", other).into())
        }
    }
}

/// A package name
#[derive(Debug, Clone)]
struct PackageName<'i> {
    span: Span,
    namespace: Name<'i>,
    name: Name<'i>,
    version: Option<(Span, Version)>,
}

impl<'i> PackageName<'i> {
    fn parse(tokens: &mut Tokenizer<'i>) -> Result<Self> {
        let namespace = Name::parse(tokens)?;
        tokens.expect(Token::Colon)?;
        let name = Name::parse(tokens)?;
        let version = parse_opt_version(tokens)?;
        Ok(PackageName {
            span: Span {
                start: namespace.span.start,
                end: version
                    .as_ref()
                    .map(|(s, _)| s.end)
                    .unwrap_or(name.span.end),
            },
            namespace,
            name,
            version,
        })
    }

    fn wit_package_name(&self) -> wit_parser::PackageName {
        wit_parser::PackageName {
            namespace: self.namespace.name.to_string(),
            name: self.name.name.to_string(),
            version: self.version.as_ref().map(|(_, v)| v.clone()),
        }
    }
}

/// e.g. foo:bar/baz@1.0
#[derive(Debug, Clone)]
pub struct Id<'i> {
    package: PackageName<'i>,
    element: Name<'i>,
}

impl<'i> Id<'i> {
    fn parse(tokens: &mut Tokenizer<'i>) -> Result<Id<'i>> {
        let namespace = Name::parse(tokens)?;
        tokens.expect(Token::Colon)?;
        let package = Name::parse(tokens)?;
        tokens.expect(Token::Slash)?;
        let element = Name::parse(tokens)?;
        let version = parse_opt_version(tokens)?;
        Ok(Id {
            package: PackageName {
                span: Span {
                    start: namespace.span.start,
                    end: package.span.end,
                },
                namespace,
                name: package,
                version,
            },
            element,
        })
    }
}

enum NameOrId<'i> {
    Id(Id<'i>),
    Name(Name<'i>),
}

impl<'i> NameOrId<'i> {
    fn parse(tokens: &mut Tokenizer<'i>) -> Result<Self> {
        let mut clone = tokens.clone();
        let first = Name::parse(tokens)?;
        if tokens.eat(Token::Colon)? {
            // We're looking at an Id, parse as such
            let id = Id::parse(&mut clone).map(NameOrId::Id)?;
            *tokens = clone;
            Ok(id)
        } else {
            Ok(NameOrId::Name(first))
        }
    }
}

/// Name is a kebab-case identifier, e.g. foo-bar.
#[derive(Debug, Clone)]
struct Name<'i> {
    name: &'i str,
    span: Span,
}

impl<'i> From<&'i str> for Name<'i> {
    fn from(s: &'i str) -> Name<'i> {
        Name {
            name: s.into(),
            span: Span { start: 0, end: 0 },
        }
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
struct Docs<'i> {
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

fn parse_opt_version(tokens: &mut Tokenizer<'_>) -> Result<Option<(Span, Version)>> {
    if !tokens.eat(Token::At)? {
        return Ok(None);
    }
    let start = tokens.expect(Token::Integer)?.start;
    tokens.expect(Token::Period)?;
    tokens.expect(Token::Integer)?;
    tokens.expect(Token::Period)?;
    let end = tokens.expect(Token::Integer)?.end;
    let mut span = Span { start, end };
    eat_ids(tokens, Token::Minus, &mut span)?;
    eat_ids(tokens, Token::Plus, &mut span)?;
    let string = tokens.get_span(span);
    let version = Version::parse(string).map_err(|e| error::Error {
        span,
        msg: e.to_string(),
    })?;
    return Ok(Some((span, version)));

    fn eat_ids(tokens: &mut Tokenizer<'_>, prefix: Token, end: &mut Span) -> Result<()> {
        if !tokens.eat(prefix)? {
            return Ok(());
        }
        loop {
            match tokens.next()? {
                Some((span, Token::Id)) | Some((span, Token::Integer)) => end.end = span.end,
                other => break Err(error::expected(tokens, "an id or integer", other).into()),
            }

            // If there's no trailing period, then this semver identifier is
            // done.
            let mut clone = tokens.clone();
            if !clone.eat(Token::Period)? {
                break Ok(());
            }

            // If there's more to the identifier, then eat the period for real
            // and continue
            if clone.eat(Token::Id)? || clone.eat(Token::Integer)? {
                tokens.eat(Token::Period)?;
                continue;
            }

            // Otherwise for something like `use foo:bar/baz@1.2.3+foo.{` stop
            // the parsing here.
            break Ok(());
        }
    }
}