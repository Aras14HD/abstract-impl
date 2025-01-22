use quote::ToTokens;
use syn::punctuated::Punctuated;
use syn::token;
use syn::{parse_macro_input, ItemImpl};
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
/// impl SomeTrait for Impl where Self: Default + std::fmt::Debug {
///   fn some() -> Self {
///     Self::default()
///   }
///   fn other(&self) -> String {
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
    let res = transform::transform(
        parsed,
        attrs.iter().all(|attr| attr.to_string() != "no_dummy"),
        attrs.iter().all(|attr| attr.to_string() != "no_macro"),
    );
    res.to_token_stream().into()
}
