use std::collections::HashSet;

use crate::{dummy::generate_dummy_impl, mac::generate_impl_macro};

use super::change_self::ChangeSelfToContext;
use proc_macro2::Span;
use quote::quote;
use syn::{
    fold::Fold,
    parse_quote,
    punctuated::Punctuated,
    spanned::Spanned,
    token::{Brace, Bracket, Comma, Mod, Paren, Pound, Pub, Where},
    AttrStyle, Attribute, Error, GenericArgument, GenericParam, Generics, Ident, ImplItem,
    ImplItemConst, ImplItemFn, ImplItemType, Item, ItemConst, ItemFn, ItemImpl, ItemMod, ItemType,
    MetaList, Path, PathArguments, ReturnType, Type, TypeParam, Visibility, WhereClause,
};

pub fn transform(
    imp: ItemImpl,
    use_dummy: bool,
    use_macro: bool,
    legacy_order: bool,
) -> syn::Result<ItemMod> {
    let copy = imp.clone();
    let ItemImpl {
        mut attrs,
        unsafety,
        generics,
        self_ty,
        items,
        trait_,
        ..
    } = imp;
    let local_idents = items
        .iter()
        .filter_map(|e| match e {
            ImplItem::Type(ty) => Some((ty.ident.clone(), (true, vec![]))),
            ImplItem::Const(c) => Some((c.ident.clone(), (true, vec![]))),
            ImplItem::Fn(f) => Some((f.sig.ident.clone(), (true, vec![]))),
            _ => None,
        })
        .collect();
    let mut folder = ChangeSelfToContext {
        local_idents,
        replaced: false,
        found_idents: std::collections::HashSet::new(),
    };

    let trait_ = trait_
        .ok_or(Error::new(copy.span(), "No trait for the impl given"))?
        .1;
    let Type::Path(syn::TypePath {
        qself: None,
        path: ty,
    }) = *self_ty
    else {
        Err(Error::new(
            self_ty.span(),
            "Impl/Trait name has to be a Path",
        ))?
    };
    let (ty, trait_) = if !legacy_order {
        (trait_, ty)
    } else {
        (ty, trait_)
    };
    (ty.segments.len() == 1)
        .then_some(())
        .ok_or(Error::new(ty.span(), "Impl names have to be Idents"))?;
    let ty_generics = match ty.segments[0].arguments.clone() {
        PathArguments::None => Punctuated::new(),
        PathArguments::AngleBracketed(args) => args.args,
        PathArguments::Parenthesized(p) => Err(Error::new(p.span(), "Impls are not functions"))?,
    };
    let ty = ty.segments[0].ident.clone();

    let mut processed: Vec<Item> = items
        .into_iter()
        .map(|item| match item {
            ImplItem::Const(c) => Ok(process_const(
                c,
                generics.clone(),
                ty_generics.clone(),
                &mut folder,
            )?),
            ImplItem::Fn(f) => Ok(process_fn(
                f,
                generics.clone(),
                ty_generics.clone(),
                &mut folder,
            )?),
            ImplItem::Type(t) => Ok(process_type(
                t,
                generics.clone(),
                ty_generics.clone(),
                &mut folder,
            )?),
            o => Err(Error::new(
                o.span(),
                "Abstract impls can only contain functions/methods and types!",
            )),
        })
        .collect::<syn::Result<_>>()?;
    processed.push(parse_quote! {use super::*;});

    attrs.push(Attribute {
        pound_token: Pound::default(),
        style: AttrStyle::Outer,
        bracket_token: Bracket::default(),
        meta: syn::Meta::List(MetaList {
            path: Path::from(Ident::new("allow", Span::call_site())),
            delimiter: syn::MacroDelimiter::Paren(Paren::default()),
            tokens: quote!(non_snake_case, type_alias_bounds),
        }),
    });

    // Dummy Impl (for errors)
    #[cfg(feature = "dummy")]
    if use_dummy {
        processed.push(parse_quote! {struct Dummy;});
        processed.push(generate_dummy_impl(
            copy.clone(),
            trait_.clone(),
            ty_generics.clone(),
        )?);
    }
    #[cfg(feature = "macro")]
    if use_macro {
        processed.push(generate_impl_macro(
            copy,
            &ty,
            &mut folder,
            trait_,
            generics.clone(),
            ty_generics.clone(),
        ));
    }

    Ok(ItemMod {
        attrs,
        vis: syn::Visibility::Public(Pub::default()),
        unsafety,
        mod_token: Mod::default(),
        ident: ty,
        content: Some((Brace::default(), processed)),
        semi: None,
    })
}

fn process_type(
    t: ImplItemType,
    append_generics: Generics,
    ty_generics: Punctuated<GenericArgument, Comma>,
    folder: &mut ChangeSelfToContext,
) -> syn::Result<Item> {
    let ImplItemType {
        attrs,
        ident,
        mut generics,
        mut ty,
        type_token,
        eq_token,
        semi_token,
        ..
    } = t;

    // change Self (to local or Context)
    folder.replaced = false;
    folder.found_idents = HashSet::new();
    ty = folder.fold_type(ty);
    generics.where_clause = None;
    generics = folder.fold_generics(generics);

    generics = process_generics(
        generics,
        false,
        append_generics.clone(),
        ty_generics.clone(),
        folder,
    )?;
    generics.where_clause = None;
    *folder
        .local_idents
        .get_mut(&ident)
        .expect("Not in local_idents! Should be impossible.") = (
        folder.replaced,
        folder
            .found_idents
            .iter()
            .filter(|id| {
                append_generics.params.iter().any(|par| match par {
                    GenericParam::Type(TypeParam { ident, .. }) => ident == *id,
                    _ => false,
                }) || ty_generics.iter().any(|arg| match arg {
                    GenericArgument::Type(Type::Path(p)) => &p.path.segments[0].ident == *id,
                    _ => false,
                })
            })
            .cloned()
            .collect(),
    );

    Ok(Item::Type(ItemType {
        attrs,
        vis: Visibility::Public(Pub::default()),
        type_token,
        ident,
        generics,
        eq_token,
        ty: Box::new(ty),
        semi_token,
    }))
}

fn process_fn(
    f: ImplItemFn,
    generics: Generics,
    ty_generics: Punctuated<GenericArgument, Comma>,
    folder: &mut ChangeSelfToContext,
) -> syn::Result<Item> {
    let ImplItemFn {
        attrs,
        mut sig,
        mut block,
        ..
    } = f;

    sig.generics = process_generics(sig.generics, true, generics, ty_generics, folder)?;
    // change Self (to local or Context)
    sig.inputs = sig
        .inputs
        .into_iter()
        .map(|inp| folder.fold_fn_arg(inp))
        .collect();
    sig.output = match sig.output {
        ReturnType::Default => ReturnType::Default,
        ReturnType::Type(arr, t) => ReturnType::Type(arr, Box::new(folder.fold_type(*t))),
    };
    block.stmts = block
        .stmts
        .into_iter()
        .map(|stmt| folder.fold_stmt(stmt))
        .collect();

    Ok(Item::Fn(ItemFn {
        attrs,
        vis: Visibility::Public(Pub::default()),
        sig,
        block: Box::new(block),
    }))
}

fn process_const(
    c: ImplItemConst,
    append_generics: Generics,
    ty_generics: Punctuated<GenericArgument, Comma>,
    folder: &mut ChangeSelfToContext,
) -> syn::Result<Item> {
    let ImplItemConst {
        attrs,
        const_token,
        ident,
        mut generics,
        colon_token,
        ty,
        eq_token,
        mut expr,
        semi_token,
        ..
    } = c;

    generics = process_generics(generics, true, append_generics, ty_generics, folder)?;
    // change Self (to local or Context)
    expr = folder.fold_expr(expr);

    Ok(Item::Const(ItemConst {
        attrs,
        vis: Visibility::Public(Pub::default()),
        const_token,
        ident,
        generics,
        colon_token,
        ty: Box::new(ty),
        eq_token,
        expr: Box::new(expr),
        semi_token,
    }))
}

fn process_generics(
    mut generics: Generics,
    insert_all: bool,
    append_generics: Generics,
    ty_generics: Punctuated<GenericArgument, Comma>,
    folder: &ChangeSelfToContext,
) -> syn::Result<Generics> {
    if let Some(mut where_clause) = append_generics.where_clause {
        if !insert_all {
            where_clause.predicates = where_clause
                .predicates
                .into_iter()
                .filter(|x| match x {
                    syn::WherePredicate::Type(syn::PredicateType {
                        bounded_ty: Type::Path(p),
                        ..
                    }) => {
                        folder.found_idents.contains(&p.path.segments[0].ident)
                            && p.qself.as_ref().map_or(true, |x| match &*x.ty {
                                Type::Path(p) => {
                                    folder.found_idents.contains(&p.path.segments[0].ident)
                                }
                                _ => true,
                            })
                    }
                    _ => true,
                })
                .collect();
        }
        generics.where_clause = Some(WhereClause {
            where_token: Where::default(),
            predicates: generics
                .where_clause
                .map(|w| w.predicates)
                .unwrap_or_default()
                .into_iter()
                .chain(where_clause.predicates)
                .collect(),
        });
    }
    // change Self (to local or Context)
    generics.params = (insert_all
        || folder
            .found_idents
            .contains(&Ident::new("Self", Span::mixed_site())))
    .then_some(syn::GenericParam::Type(TypeParam::from(Ident::new(
        "Context",
        Span::mixed_site(),
    ))))
    .into_iter()
    .map(Ok)
    .chain(
        ty_generics
            .into_iter()
            .map(|arg| match arg {
                GenericArgument::Lifetime(l) => Ok(GenericParam::Lifetime(syn::LifetimeParam {
                    attrs: vec![],
                    lifetime: l,
                    colon_token: None,
                    bounds: Punctuated::new(),
                })),
                GenericArgument::Type(Type::Path(p)) => Ok(GenericParam::Type(syn::TypeParam {
                    attrs: vec![],
                    ident: p.path.segments[0].ident.clone(),
                    colon_token: None,
                    bounds: Punctuated::new(),
                    eq_token: None,
                    default: None,
                })),
                o => Err(Error::new(
                    o.span(),
                    "Only Type and Lifetime generics are supported on Impl",
                )),
            })
            .filter(|param| match param {
                Ok(GenericParam::Type(TypeParam { ident, .. })) => {
                    folder.found_idents.contains(ident) || insert_all
                }
                _ => true,
            }),
    )
    .chain(append_generics.params.into_iter().map(Ok))
    .chain(generics.params.into_iter().map(|param| match param {
        GenericParam::Type(mut t) => {
            t.default = t.default.map(|d| folder.clone().fold_type(d));
            Ok(GenericParam::Type(t))
        }
        other => Ok(other),
    }))
    .collect::<syn::Result<_>>()?;
    generics.where_clause = generics.where_clause.map(|mut w| {
        w.predicates = w
            .predicates
            .into_iter()
            .map(|pred| folder.clone().fold_where_predicate(pred))
            .collect();
        w
    });
    Ok(generics)
}
