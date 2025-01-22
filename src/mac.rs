use proc_macro2::Span;
use quote::quote;
use syn::{
    punctuated::Punctuated,
    spanned::Spanned,
    token::{Brace, Bracket, Comma, Gt, Lt, Not, Paren, PathSep, Pound},
    AngleBracketedGenericArguments, AttrStyle, Attribute, Block, Expr, ExprConst, ExprPath,
    FieldPat, FnArg, GenericArgument, GenericParam, Generics, Ident, ImplItem, Item, ItemImpl,
    ItemMacro, Pat, PatIdent, PatOr, PatParen, PatReference, PatSlice, PatStruct, PatTuple,
    PatTupleStruct, PatType, Path, PathArguments, PathSegment, Receiver, Stmt, Type, TypePath,
};

use crate::change_self::ChangeSelfToContext;

pub fn generate_impl_macro(
    imp: ItemImpl,
    ty: &Ident,
    folder: &mut ChangeSelfToContext,
    trait_: Path,
) -> Item {
    let items = imp
        .items
        .into_iter()
        .map(|item| match item {
            ImplItem::Const(mut c) => {
                c.ty = Type::Path(TypePath {
                    qself: None,
                    path: Path {
                        leading_colon: None,
                        segments: [
                            PathSegment {
                                ident: ty.clone(),
                                arguments: PathArguments::None,
                            },
                            PathSegment {
                                ident: c.ident.clone(),
                                arguments: PathArguments::AngleBracketed(
                                    AngleBracketedGenericArguments {
                                        colon2_token: None,
                                        lt_token: Lt::default(),
                                        args: generic_to_arg(c.generics.clone(), true),
                                        gt_token: Gt::default(),
                                    },
                                ),
                            },
                        ]
                        .into_iter()
                        .collect(),
                    },
                });
                ImplItem::Const(c)
            }
            ImplItem::Fn(mut f) => {
                let args = f
                    .sig
                    .inputs
                    .iter()
                    .map(|inp| match inp {
                        FnArg::Receiver(Receiver { self_token, .. }) => Expr::Path(ExprPath {
                            attrs: vec![],
                            qself: None,
                            path: Path::from(Ident::new("self", self_token.span())),
                        }),
                        FnArg::Typed(PatType { pat, .. }) => pat_to_expr(*pat.clone()).remove(0),
                    })
                    .collect();
                f.block.stmts = vec![Stmt::Expr(
                    Expr::Call(syn::ExprCall {
                        attrs: vec![],
                        func: Box::new(Expr::Path(ExprPath {
                            attrs: vec![],
                            qself: None,
                            path: Path {
                                leading_colon: None,
                                segments: [
                                    PathSegment {
                                        ident: ty.clone(),
                                        arguments: PathArguments::None,
                                    },
                                    PathSegment {
                                        ident: f.sig.ident.clone(),
                                        arguments: PathArguments::AngleBracketed(
                                            AngleBracketedGenericArguments {
                                                colon2_token: Some(PathSep::default()),
                                                lt_token: Lt::default(),
                                                args: generic_to_arg(f.sig.generics.clone(), true),
                                                gt_token: Gt::default(),
                                            },
                                        ),
                                    },
                                ]
                                .into_iter()
                                .collect(),
                            },
                        })),
                        paren_token: Paren::default(),
                        args,
                    }),
                    None,
                )];
                ImplItem::Fn(f)
            }
            ImplItem::Type(mut t) => {
                t.ty = Type::Path(TypePath {
                    qself: None,
                    path: Path {
                        leading_colon: None,
                        segments: [
                            PathSegment {
                                ident: ty.clone(),
                                arguments: PathArguments::None,
                            },
                            PathSegment {
                                ident: t.ident.clone(),
                                arguments: {
                                    let args = generic_to_arg(
                                        t.generics.clone(),
                                        folder
                                            .local_idents
                                            .iter()
                                            .find(|i| i.0 == t.ident)
                                            .map(|i| i.1)
                                            .unwrap_or(false),
                                    );
                                    if args.is_empty() {
                                        PathArguments::None
                                    } else {
                                        PathArguments::AngleBracketed(
                                            AngleBracketedGenericArguments {
                                                colon2_token: Some(PathSep::default()),
                                                lt_token: Lt::default(),
                                                args,
                                                gt_token: Gt::default(),
                                            },
                                        )
                                    }
                                },
                            },
                        ]
                        .into_iter()
                        .collect(),
                    },
                });
                ImplItem::Type(t)
            }
            other => other,
        })
        .collect::<Box<_>>();
    Item::Macro(ItemMacro {
        attrs: vec![Attribute {
            pound_token: Pound::default(),
            style: AttrStyle::Outer,
            bracket_token: Bracket::default(),
            meta: syn::Meta::Path(Path::from(Ident::new("macro_export", Span::call_site()))),
        }],
        ident: Some(Ident::new(&format!("impl_{}", ty), Span::call_site())),
        mac: syn::Macro {
            path: Path::from(Ident::new("macro_rules", Span::call_site())),
            bang_token: Not::default(),
            delimiter: syn::MacroDelimiter::Brace(Brace::default()),
            tokens: quote! {
                ($t:ty) => {
                    impl #trait_ for $t {
                        #(#items)*
                    }
                };
            },
        },
        semi_token: None,
    })
}

fn generic_to_arg(generics: Generics, prepend_self: bool) -> Punctuated<GenericArgument, Comma> {
    prepend_self
        .then_some(GenericArgument::Type(Type::Path(TypePath {
            qself: None,
            path: Path::from(Ident::new("Self", Span::call_site())),
        })))
        .into_iter()
        .chain(
            generics
                .params
                .clone()
                .into_iter()
                .map(|param| match param {
                    GenericParam::Lifetime(l) => GenericArgument::Lifetime(l.lifetime),
                    GenericParam::Type(t) => GenericArgument::Type(Type::Path(TypePath {
                        qself: None,
                        path: Path::from(t.ident),
                    })),
                    GenericParam::Const(c) => GenericArgument::Const(Expr::Const(ExprConst {
                        attrs: vec![],
                        const_token: c.const_token,
                        block: Block {
                            brace_token: Brace::default(),
                            stmts: vec![Stmt::Expr(
                                Expr::Path(ExprPath {
                                    attrs: vec![],
                                    qself: None,
                                    path: Path::from(c.ident),
                                }),
                                None,
                            )],
                        },
                    })),
                }),
        )
        .collect()
}

fn pat_to_expr(pat: Pat) -> Vec<Expr> {
    match pat {
        Pat::Const(c) => vec![Expr::Const(c)],
        Pat::Ident(PatIdent { ident, .. }) => vec![Expr::Path(ExprPath {
            attrs: vec![],
            qself: None,
            path: Path::from(ident),
        })],
        Pat::Lit(l) => vec![Expr::Lit(l)],
        Pat::Macro(m) => vec![Expr::Macro(m)],
        Pat::Or(PatOr { .. }) => todo!(),
        Pat::Paren(PatParen { pat, .. }) => pat_to_expr(*pat),
        Pat::Path(p) => vec![Expr::Path(p)],
        Pat::Range(r) => vec![Expr::Range(r)],
        Pat::Reference(PatReference { pat, .. }) => pat_to_expr(*pat),
        Pat::Rest(_) => vec![],
        Pat::Slice(PatSlice { elems, .. }) => {
            elems.into_iter().flat_map(|pat| pat_to_expr(pat)).collect()
        }
        Pat::Struct(PatStruct { fields, .. }) => fields
            .into_iter()
            .flat_map(|FieldPat { pat, .. }| pat_to_expr(*pat))
            .collect(),
        Pat::Tuple(PatTuple { elems, .. }) => {
            elems.into_iter().flat_map(|pat| pat_to_expr(pat)).collect()
        }
        Pat::TupleStruct(PatTupleStruct { elems, .. }) => {
            elems.into_iter().flat_map(|pat| pat_to_expr(pat)).collect()
        }
        Pat::Type(PatType { pat, .. }) => pat_to_expr(*pat),
        Pat::Verbatim(v) => vec![Expr::Verbatim(v)],
        _ => vec![],
    }
}
