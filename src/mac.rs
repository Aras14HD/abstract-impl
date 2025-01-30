use std::collections::HashMap;

use proc_macro2::Span;
use quote::{quote, ToTokens};
use syn::{
    fold::Fold,
    punctuated::Punctuated,
    spanned::Spanned,
    token::{Brace, Bracket, Comma, Gt, Lt, Not, Paren, PathSep, Pound},
    AngleBracketedGenericArguments, AttrStyle, Attribute, Block, Expr, ExprConst, ExprPath,
    FieldPat, FnArg, GenericArgument, GenericParam, Generics, Ident, ImplItem, Item, ItemImpl,
    ItemMacro, Pat, PatIdent, PatOr, PatParen, PatReference, PatSlice, PatStruct, PatTuple,
    PatTupleStruct, PatType, Path, PathArguments, PathSegment, Receiver, Stmt, Type, TypeParam,
    TypePath,
};

use crate::change_self::ChangeSelfToContext;

pub fn generate_impl_macro(
    imp: ItemImpl,
    ty: &Ident,
    folder: &mut ChangeSelfToContext,
    trait_: Path,
    generics: Generics,
    ty_generics: Punctuated<GenericArgument, Comma>,
) -> Item {
    let external_types: Box<[_]> = ty_generics
        .iter()
        .map(|x| {
            Expr::Verbatim(
                format!("${}:ty", x.into_token_stream().to_string().to_lowercase())
                    .parse()
                    .unwrap(),
            )
        })
        .collect();
    let external_types_use: Box<[_]> = ty_generics
        .iter()
        .map(|x| {
            Expr::Verbatim(
                format!("${}", x.into_token_stream().to_string().to_lowercase())
                    .parse()
                    .unwrap(),
            )
        })
        .collect();
    let new_ty_generics = ty_generics
        .iter()
        .cloned()
        .map(|mut x| {
            if let GenericArgument::Type(Type::Path(p)) = &mut x {
                p.path.segments[0].ident = Ident::new(
                    &format!("_impl_{ty}_{}", p.path.segments[0].ident),
                    p.path.segments[0].ident.span(),
                )
            }
            x
        })
        .collect::<Box<_>>();
    let items = imp
        .items
        .into_iter()
        .map(|item| match item {
            ImplItem::Const(c) => {
                generate_const(c, ty.clone(), generics.clone(), new_ty_generics.clone())
            }
            ImplItem::Fn(f) => {
                generate_fn(f, ty.clone(), generics.clone(), new_ty_generics.clone())
            }
            ImplItem::Type(t) => generate_type(
                t,
                ty.clone(),
                generics.clone(),
                new_ty_generics.clone(),
                folder,
            ),
            other => other,
        })
        .collect::<Box<_>>();
    let gens = generics.params;
    let mut replace_ident = ReplaceIdents(
        ty_generics
            .iter()
            .zip(new_ty_generics.iter())
            .filter_map(|args| match args {
                (
                    GenericArgument::Type(Type::Path(old)),
                    GenericArgument::Type(Type::Path(new)),
                ) => Some((
                    old.path.segments[0].ident.to_string(),
                    new.path.segments[0].ident.to_string(),
                )),
                _ => None,
            })
            .collect(),
    );
    let where_clause = generics
        .where_clause
        .map(|w| replace_ident.fold_where_clause(w));
    let trait_ = replace_ident.fold_path(trait_);
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
            tokens: if external_types.is_empty() {
                quote! {
                    ($t:ty) => {
                        impl<#gens> #trait_ for $t #where_clause {
                            #(#items)*
                        }
                    };
                }
            } else {
                quote! {
                    (<#(#external_types),*> $ty:ty) => {
                        #(#[allow(non_camel_case_types)]type #new_ty_generics = #external_types_use;)*
                        impl<#gens> #trait_ for $ty #where_clause {
                            #(#items)*
                        }
                    }
                }
            },
        },
        semi_token: None,
    })
}

fn generate_type(
    mut t: syn::ImplItemType,
    ty: Ident,
    mut generics: Generics,
    mut ty_generics: Box<[GenericArgument]>,
    folder: &mut ChangeSelfToContext,
) -> ImplItem {
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
                        let (has_context, retained_generics) =
                            folder.local_idents.get(&t.ident).unwrap();
                        generics.params = generics
                            .params
                            .into_iter()
                            .filter(|gen| match gen {
                                GenericParam::Type(TypeParam { ident, .. }) => {
                                    retained_generics.contains(ident)
                                }
                                _ => true,
                            })
                            .collect();
                        ty_generics = ty_generics
                            .into_vec()
                            .into_iter()
                            .filter(|gen| match gen {
                                GenericArgument::Type(Type::Path(p)) => {
                                    retained_generics.iter().any(|id| {
                                        p.path.segments[0].ident == format!("_impl_{ty}_{}", id)
                                    })
                                }
                                _ => true,
                            })
                            .collect();
                        let args =
                            generic_to_arg(t.generics.clone(), *has_context, generics, ty_generics);
                        if args.is_empty() {
                            PathArguments::None
                        } else {
                            PathArguments::AngleBracketed(AngleBracketedGenericArguments {
                                colon2_token: Some(PathSep::default()),
                                lt_token: Lt::default(),
                                args,
                                gt_token: Gt::default(),
                            })
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

fn generate_fn(
    mut f: syn::ImplItemFn,
    ty: Ident,
    generics: Generics,
    ty_generics: Box<[GenericArgument]>,
) -> ImplItem {
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
                            ident: ty,
                            arguments: PathArguments::None,
                        },
                        PathSegment {
                            ident: f.sig.ident.clone(),
                            arguments: PathArguments::AngleBracketed(
                                AngleBracketedGenericArguments {
                                    colon2_token: Some(PathSep::default()),
                                    lt_token: Lt::default(),
                                    args: generic_to_arg(
                                        f.sig.generics.clone(),
                                        true,
                                        generics,
                                        ty_generics,
                                    ),
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

fn generate_const(
    mut c: syn::ImplItemConst,
    ty: Ident,
    generics: Generics,
    ty_generics: Box<[GenericArgument]>,
) -> ImplItem {
    c.ty = Type::Path(TypePath {
        qself: None,
        path: Path {
            leading_colon: None,
            segments: [
                PathSegment {
                    ident: ty,
                    arguments: PathArguments::None,
                },
                PathSegment {
                    ident: c.ident.clone(),
                    arguments: PathArguments::AngleBracketed(AngleBracketedGenericArguments {
                        colon2_token: None,
                        lt_token: Lt::default(),
                        args: generic_to_arg(c.generics.clone(), true, generics, ty_generics),
                        gt_token: Gt::default(),
                    }),
                },
            ]
            .into_iter()
            .collect(),
        },
    });
    ImplItem::Const(c)
}

fn generic_to_arg(
    generics: Generics,
    prepend_self: bool,
    append_generics: Generics,
    ty_generics: Box<[GenericArgument]>,
) -> Punctuated<GenericArgument, Comma> {
    prepend_self
        .then_some(GenericArgument::Type(Type::Path(TypePath {
            qself: None,
            path: Path::from(Ident::new("Self", Span::call_site())),
        })))
        .into_iter()
        .chain(ty_generics)
        .chain(append_generics.params.into_iter().map(|param| match param {
            GenericParam::Lifetime(l) => GenericArgument::Lifetime(l.lifetime),
            GenericParam::Type(t) => GenericArgument::Type(Type::Path(TypePath {
                qself: None,
                path: Path::from(t.ident),
            })),
            GenericParam::Const(c) => GenericArgument::Const(Expr::Path(ExprPath {
                attrs: vec![],
                qself: None,
                path: Path::from(c.ident),
            })),
        }))
        .chain(
            generics
                .params
                .clone()
                .into_iter()
                .filter_map(|param| match param {
                    GenericParam::Lifetime(_) => None,
                    GenericParam::Type(t) => Some(GenericArgument::Type(Type::Path(TypePath {
                        qself: None,
                        path: Path::from(t.ident),
                    }))),
                    GenericParam::Const(c) => {
                        Some(GenericArgument::Const(Expr::Const(ExprConst {
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
                        })))
                    }
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
        Pat::Slice(PatSlice { elems, .. }) => elems.into_iter().flat_map(pat_to_expr).collect(),
        Pat::Struct(PatStruct { fields, .. }) => fields
            .into_iter()
            .flat_map(|FieldPat { pat, .. }| pat_to_expr(*pat))
            .collect(),
        Pat::Tuple(PatTuple { elems, .. }) => elems.into_iter().flat_map(pat_to_expr).collect(),
        Pat::TupleStruct(PatTupleStruct { elems, .. }) => {
            elems.into_iter().flat_map(pat_to_expr).collect()
        }
        Pat::Type(PatType { pat, .. }) => pat_to_expr(*pat),
        Pat::Verbatim(v) => vec![Expr::Verbatim(v)],
        _ => vec![],
    }
}

struct ReplaceIdents(HashMap<String, String>);

impl syn::fold::Fold for ReplaceIdents {
    fn fold_ident(&mut self, i: proc_macro2::Ident) -> proc_macro2::Ident {
        let Some(out) = self.0.get(&i.to_string()) else {
            return i;
        };
        Ident::new(out, i.span())
    }
}
