use core::panic;

use proc_macro2::Span;
use syn::{
    parse_quote,
    punctuated::Punctuated,
    token::{Brace, Comma, For, Paren},
    Block, GenericArgument, Ident, ImplItem, Item, ItemImpl, LifetimeParam, Path, Stmt, Type,
    TypePath, TypeTuple,
};

pub fn generate_dummy_impl(
    mut imp: ItemImpl,
    trait_: Path,
    ty_generics: Punctuated<GenericArgument, Comma>,
) -> Item {
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
            GenericArgument::Lifetime(l) => syn::GenericParam::Lifetime(syn::LifetimeParam {
                attrs: vec![],
                lifetime: l,
                colon_token: None,
                bounds: Punctuated::new(),
            }),
            GenericArgument::Type(Type::Path(p)) => syn::GenericParam::Type(syn::TypeParam {
                attrs: vec![],
                ident: p.path.segments[0].ident.clone(),
                colon_token: None,
                bounds: Punctuated::new(),
                eq_token: None,
                default: None,
            }),
            _ => panic!("Impl cannot have generics other than type or Lifetime"),
        })
        .chain(imp.generics.params)
        .collect();
    Item::Impl(imp)
}
