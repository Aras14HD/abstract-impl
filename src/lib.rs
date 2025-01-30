#![allow(clippy::needless_doctest_main)]
#![doc = include_str!("../README.md")]
use core::panic;

use quote::{quote, ToTokens};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{parse_macro_input, ExprRepeat, ItemImpl, ItemTrait, Path, TraitItemType};
use syn::{token, Ident};
mod change_self;
mod dummy;
mod mac;
mod transform;

struct IdentList(Punctuated<syn::Ident, token::Comma>);
impl syn::parse::Parse for IdentList {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        Ok(IdentList(
            input.parse_terminated(syn::Ident::parse, token::Comma)?,
        ))
    }
}

/// Define an abstract implementation for a trait, that types can use
///
/// ```
/// # use abstract_impl::abstract_impl;
/// # trait SomeTrait {
/// #   fn some() -> Self;
/// #   fn other(&self) -> String;
/// # }
/// #[abstract_impl]
/// impl Impl for SomeTrait where Self: Default + std::fmt::Debug {
///   fn some() -> Self {
///     Self::default()
///   }
///   fn other(&self) -> String {
///     // You have to use context here instead of self, because we don't change macro contents
///     format!("{context:?}")
///   }
/// }
/// impl_Impl!(());
/// # fn main() {
/// # assert_eq!("()", <()>::some().other());
/// # }
/// ```
#[proc_macro_attribute]
pub fn abstract_impl(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let parsed = parse_macro_input!(item as ItemImpl);
    let IdentList(attrs) = parse_macro_input!(_attr);
    let res = match transform::transform(
        parsed,
        attrs.iter().all(|attr| *attr != "no_dummy"),
        attrs.iter().all(|attr| *attr != "no_macro"),
        attrs.iter().any(|attr| *attr == "legacy_order"),
    ) {
        Ok(res) => res,
        Err(e) => return e.into_compile_error().into(),
    };
    res.to_token_stream().into()
}

/// Generates a TyType trait (has type Ty) with a generic TyUsingType<T> impl given a type name Ty.
/// ```rust
/// # use abstract_impl::type_trait;
/// type_trait!(Ty);
/// struct Test;
/// impl_TyUsingType!(<u8> Test);
/// fn main() {
///   let x: <Test as TyType>::Ty = 5u8;
/// # assert_eq!(x, 5);
/// }
/// ```
#[proc_macro]
pub fn type_trait(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ty = parse_macro_input!(item as Ident);
    let trait_name = Ident::new(&format!("{ty}Type"), ty.span());
    let impl_name = Ident::new(&format!("{ty}UsingType"), ty.span());
    quote! {
        pub trait #trait_name {
            type #ty;
        }
        #[::abstract_impl::abstract_impl(no_dummy)]
        impl #impl_name<T> for #trait_name {
            type #ty = T;
        }
    }
    .into()
}

/// ```rust
/// # use abstract_impl::use_type;
/// #[use_type]
/// trait SomeTrait {
///   type Ty: Default;
///   fn get_ty() -> Self::Ty {
///     Self::Ty::default()
///   }
/// }
/// # fn main() {}
/// ```
#[proc_macro_attribute]
pub fn use_type(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    use syn::ImplItem;
    use syn::TraitItem;
    use syn::Type;
    let trait_ = parse_macro_input!(item as ItemTrait);
    let name = trait_.ident.clone();
    let items = trait_
        .items
        .clone()
        .into_iter()
        .filter_map(|item| match item {
            TraitItem::Type(syn::TraitItemType {
                attrs,
                type_token,
                ident,
                generics,
                semi_token,
                ..
            }) => {
                let new_ident = Ident::new(&format!("_use_type_{ident}"), ident.span());
                Some(Ok((
                    ImplItem::Type(syn::ImplItemType {
                        attrs,
                        vis: syn::Visibility::Inherited,
                        defaultness: None,
                        type_token,
                        ident,
                        generics,
                        eq_token: syn::token::Eq::default(),
                        ty: Type::Path(syn::TypePath {
                            qself: None,
                            path: Path::from(new_ident.clone()),
                        }),
                        semi_token,
                    }),
                    new_ident,
                )))
            }
            TraitItem::Const(syn::TraitItemConst {
                attrs,
                const_token,
                ident,
                generics,
                colon_token,
                ty,
                semi_token,
                ..
            }) => {
                let new_ident = Ident::new(&format!("_use_type_{ident}"), ident.span());
                Some(Ok((
                    ImplItem::Const(syn::ImplItemConst {
                        attrs,
                        vis: syn::Visibility::Inherited,
                        defaultness: None,
                        const_token,
                        ident,
                        generics,
                        colon_token,
                        ty,
                        eq_token: syn::token::Eq::default(),
                        expr: syn::Expr::Path(syn::ExprPath {
                            attrs: vec![],
                            qself: None,
                            path: Path::from(new_ident.clone()),
                        }),
                        semi_token,
                    }),
                    new_ident,
                )))
            }
            TraitItem::Fn(syn::TraitItemFn {
                default: Some(_), ..
            }) => None,
            o => Some(Err(syn::Error::new(o.span(), "cannot implement functions"))),
        })
        .collect::<Result<(Vec<_>, Vec<_>), _>>();
    let (items, item_names) = match items {
        Ok(items) => items,
        Err(err) => return err.into_compile_error().into(),
    };
    let impl_name = Ident::new(&format!("{name}UsingType"), name.span());
    quote! {
        #trait_
        #[allow(non_camel_case_types)]
        #[::abstract_impl::abstract_impl]
        impl #impl_name<#(#item_names),*> for #name {
            #(#items)*
        }
    }
    .into()
}

/// ```rust
/// # use abstract_impl::use_field;
/// #[use_field]
/// trait SomeTrait {
///   fn field(&mut self) -> &mut str;
/// }
/// struct Test {
///   field: String,
/// }
/// impl_SomeTrait_with_field!(Test {field});
/// ```
#[proc_macro_attribute]
pub fn use_field(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    use syn::ImplItem;
    use syn::TraitItem;
    let trait_ = parse_macro_input!(item as ItemTrait);
    let name = trait_.ident.clone();
    let macro_name = Ident::new(&format!("impl_{name}_with_field"), name.span());
    let items = trait_.items.clone().into_iter().map(|item| match item {
        TraitItem::Fn(syn::TraitItemFn { attrs, sig, .. }) => {
            let (ref_, mut_) = match sig.inputs.first() {
                Some(syn::FnArg::Receiver(r)) => (r.reference.clone(), r.mutability.clone()),
                _ => panic!("All functions must take self"),
            };
            ImplItem::Fn(syn::ImplItemFn {
                attrs,
                vis: syn::Visibility::Inherited,
                defaultness: None,
                sig,
                block: syn::Block {
                    brace_token: syn::token::Brace::default(),
                    stmts: vec![syn::Stmt::Expr(
                        syn::Expr::Verbatim(match (ref_, mut_) {
                            (Some((ref_, _)), Some(mut_)) => quote! {#ref_ #mut_ self.$e},
                            (Some((ref_, _)), None) => quote! {#ref_ self.$e},
                            _ => quote! {self.$e},
                        }),
                        None,
                    )],
                },
            })
        }
        _ => panic!("Only functions can be used with use_field"),
    });
    quote! {
        #trait_
        #[macro_export]
        macro_rules! #macro_name {
            ($t:ty {$e:ident}) => {
                impl #name for $t {
                    #(#items)*
                }
            }
        }
    }
    .into()
}

#[allow(dead_code)]
struct ImplWithFieldInput {
    lt_token: syn::token::Lt,
    type_: syn::Type,
    gt_token: syn::token::Gt,
    self_: syn::Type,
    brace_token: syn::token::Brace,
    expr: syn::Expr,
}
impl syn::parse::Parse for ImplWithFieldInput {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let content;
        Ok(Self {
            lt_token: input.parse()?,
            type_: input.parse()?,
            gt_token: input.parse()?,
            self_: input.parse()?,
            brace_token: syn::braced!(content in input),
            expr: content.parse()?,
        })
    }
}

/// Implement AsRef using a field
/// ```rust
/// # use abstract_impl::*;
/// struct Test {
///   field: String,
/// };
/// impl_as_ref_with_field!(<String> Test {field});
/// ```
#[proc_macro]
pub fn impl_as_ref_with_field(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ImplWithFieldInput {
        type_, self_, expr, ..
    } = parse_macro_input!(item as ImplWithFieldInput);
    quote! {
        impl AsRef<#type_> for #self_ {
            fn as_ref(&self) -> &#type_ {
                &self.#expr
            }
        }
    }
    .into()
}
/// Implement AsMut using a field
/// ```rust
/// # use abstract_impl::*;
/// struct Test {
///   field: String,
/// };
/// impl_as_mut_with_field!(<String> Test {field});
/// ```
#[proc_macro]
pub fn impl_as_mut_with_field(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ImplWithFieldInput {
        type_, self_, expr, ..
    } = parse_macro_input!(item as ImplWithFieldInput);
    quote! {
        impl AsMut<#type_> for #self_ {
            fn as_mut(&mut self) -> &mut #type_ {
                &mut self.#expr
            }
        }
    }
    .into()
}
/// Implement Into using a field
/// ```rust
/// # use abstract_impl::*;
/// struct Test {
///   field: String,
/// };
/// impl_into_with_field!(<String> Test {field});
/// ```
#[proc_macro]
pub fn impl_into_with_field(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ImplWithFieldInput {
        type_, self_, expr, ..
    } = parse_macro_input!(item as ImplWithFieldInput);
    quote! {
        impl Into<#type_> for #self_ {
            fn into(self) -> #type_ {
                self.#expr
            }
        }
    }
    .into()
}
/// Implement Into, AsRef and AsMut using a field
/// ```rust
/// # use abstract_impl::*;
/// struct Test {
///   field: String,
/// };
/// impl_conversion_with_field!(<String> Test {field});
/// ```
#[proc_macro]
pub fn impl_conversion_with_field(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let item: proc_macro2::TokenStream = item.into();
    quote! {
        ::abstract_impl::impl_into_with_field!(#item);
        ::abstract_impl::impl_as_ref_with_field!(#item);
        ::abstract_impl::impl_as_mut_with_field!(#item);
    }
    .into()
}

// DOCTESTS(hidden):
/// ```rust
/// use abstract_impl::abstract_impl;
/// trait TimeType {
///   type Time;
/// }
/// #[abstract_impl(no_dummy)]
/// impl TimeUsingType<T> for TimeType {
///   type Time = T;
/// }
/// impl_TimeUsingType!(<usize> ());
/// fn main() {}
/// ```
#[allow(dead_code)]
struct Tests;
