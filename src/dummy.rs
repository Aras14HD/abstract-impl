use proc_macro2::Span;
use quote::quote;
use syn::{
    parse_quote,
    punctuated::Punctuated,
    spanned::Spanned,
    token::{Brace, Comma, For, Paren},
    Block, Error, GenericArgument, Ident, ImplItem, Item, ItemImpl, Path, Stmt, Type, TypePath,
    TypeTuple,
};

pub fn generate_dummy_impl(
    mut imp: ItemImpl,
    trait_: Path,
    ty_generics: Punctuated<GenericArgument, Comma>,
) -> syn::Result<Item> {
    imp.self_ty = Box::new(Type::Path(TypePath {
        qself: None,
        path: Path::from(Ident::new("Dummy", Span::call_site())),
    }));
    imp.trait_ = Some((None, trait_, For::default()));

    let dummy_body: syn::Expr = parse_quote! {
        unreachable!()
    };
    imp.items = imp
        .items
        .into_iter()
        .map(|item| match item {
            ImplItem::Fn(mut f) => {
                f.block = Block {
                    brace_token: Brace::default(),
                    stmts: vec![Stmt::Expr(dummy_body.clone(), None)],
                };
                ImplItem::Fn(f)
            }
            ImplItem::Const(mut c) => {
                c.expr = dummy_body.clone();
                ImplItem::Const(c)
            }
            ImplItem::Type(mut t) => {
                t.ty = Type::Tuple(TypeTuple {
                    paren_token: Paren::default(),
                    elems: Punctuated::new(),
                });
                ImplItem::Type(t)
            }
            other => other,
        })
        .collect();
    imp.generics.where_clause = None;
    imp.generics.params = ty_generics
        .into_iter()
        .map(|arg| match arg {
            GenericArgument::Lifetime(l) => Ok(syn::GenericParam::Lifetime(syn::LifetimeParam {
                attrs: vec![],
                lifetime: l,
                colon_token: None,
                bounds: Punctuated::new(),
            })),
            GenericArgument::Type(Type::Path(p)) => Ok(syn::GenericParam::Type(syn::TypeParam {
                attrs: vec![],
                ident: p.path.segments[0].ident.clone(),
                colon_token: None,
                bounds: Punctuated::new(),
                eq_token: None,
                default: None,
            })),
            o => Err(Error::new(
                o.span(),
                "Impl cannot have generics other than type or Lifetime",
            )),
        })
        .chain(imp.generics.params.into_iter().map(Ok))
        .collect::<syn::Result<_>>()?;
    imp.attrs.push(syn::Attribute {
        pound_token: syn::token::Pound::default(),
        style: syn::AttrStyle::Outer,
        bracket_token: syn::token::Bracket::default(),
        meta: syn::Meta::List(syn::MetaList {
            path: Path::from(Ident::new("allow", Span::mixed_site())),
            delimiter: syn::MacroDelimiter::Paren(syn::token::Paren::default()),
            tokens: quote! {unused_variables},
        }),
    });
    Ok(Item::Impl(imp))
}
