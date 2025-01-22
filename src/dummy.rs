use proc_macro2::Span;
use syn::{
    parse_quote,
    punctuated::Punctuated,
    token::{Brace, For, Paren},
    Block, Ident, ImplItem, Item, ItemImpl, Path, Stmt, Type, TypePath, TypeTuple,
};

pub fn generate_dummy_impl(mut imp: ItemImpl, trait_: Path) -> Item {
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
    Item::Impl(imp)
}
