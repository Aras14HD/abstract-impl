# Abstract Impl
This crate enables users to generate generic (abstract) implementations for traits, with bounds on the Self type.

The idea for this came primarily from [Context-Generic Programming](https://contextgeneric.dev/),
I wanted the core without all the bells and whistles (no providers, consumers and contexts).
I noticed that it primarily empowers the programmer to provide generic implementations for traits, that types can choose to implement.

The name abstract_impl comes from the possibility of having that in rust (abstract is a reserved keyword), you could have `abstract impl`.

## Use
```rust
use abstract_impl::abstract_impl;
// Create an implementation
#[abstract_impl]
impl DebugToString for ToString where Self: std::fmt::Debug {
                    // Require Bounds ~~~~~~~~~~~~~~~~~~~~~
    fn to_string(&self) -> String {
        //       ~~~~~     ~~~~~~
        // Use self like normal
        format!("{context:?}")
        //        ~~~~~~~
        // Sometimes (in macro invocations) you have to use context
    }
}
#[derive(Debug)]
struct Test(pub u8);
impl_DebugToString!(Test);
```
Features:
 - type errors at the right place
 - check that all items of the trait are implemented
 - use of self type with bounds
 - automatic implementation macro
### A more elaborate showcase
```rust
use abstract_impl::abstract_impl;
mod traits {
    pub trait FormatToString {
        type Error;
        fn format_to_string(&self) -> Result<String, Self::Error>;
    }

    pub trait Print {
        type Error;
        fn print(&self) -> Result<(), Self::Error>;
    }
}
use traits::*;

mod impls {
    use super::*;

    #[abstract_impl]
    impl FormatUsingDebug for FormatToString
    where
        Self: std::fmt::Debug,
    {
        type Error = ();
        fn format_to_string(&self) -> Result<String, Self::Error> {
            Ok(format!("{context:?}"))
        }
    }

    #[derive(Debug)]
    pub enum PrintUsingFormatErr<FormatErr> {
        Other,
        FormatErr(FormatErr),
    }
    #[abstract_impl]
    impl PrintUsingFormat for Print
    where
        Self: FormatToString,
    {
        type Error = PrintUsingFormatErr<<Self as FormatToString>::Error>;
        fn print(&self) -> Result<(), Self::Error> {
            let string = self.format_to_string().map_err(Self::Error::FormatErr)?;
            Ok(println!("{string}"))
        }
    }
    #[abstract_impl]
    impl PrintDummy for Print {
        type Error = ();
        fn print(&self) -> Result<(), ()> {
            Ok(println!("Hi"))
        }
    }
    #[abstract_impl(no_dummy)]
    impl PartialUsingOrd for std::cmp::PartialOrd
    where
        Self: Ord,
    {
        fn partial_cmp(&self, rhs: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(rhs))
        }
    }
}
use impls::*;

#[derive(Debug)]
struct Person {
    pub first_name: String,
    pub last_name: String,
}

impl_FormatUsingDebug!(Person);
impl_PrintUsingFormat!(Person);

fn main() {
    Person {
        first_name: "Alice".to_string(),
        last_name: "B.".to_string(),
    }
    .print()
    .unwrap();
}
```
### Trait-Generics
You can make implementations over generic traits.
```rust
use abstract_impl::abstract_impl;
trait HasType<T> {
    fn get_type(self) -> T;
}
trait FormatType<T> {
    fn format_type(self) -> String;
}
#[abstract_impl]
impl<T> FormatField for FormatType<T> where Self: HasType<T>, T: ToString {
    fn format_type(self) -> String {
        format!("{}", context.get_type().to_string())
    }
}
struct Test(u8);
impl HasType<u8> for Test {
    fn get_type(self) -> u8 {
        self.0
    }
}
impl_FormatField!(Test);
fn main() {
    let t = Test(5);
    assert_eq!("5", t.format_type());
}
```
### Impl-Generics
Or you can make the impl (macro) take generic parameters.
```rust
use abstract_impl::abstract_impl;
trait HasType<T> {
    fn get_type(self) -> T;
}
trait FormatType<T> {
    fn format_type(self) -> String;
}
#[abstract_impl]
impl FormatField<T> for FormatType<T> where Self: HasType<T>, T: ToString {
    fn format_type(self) -> String {
        format!("{}", context.get_type().to_string())
    }
}
struct Test(u8);
impl HasType<u8> for Test {
    fn get_type(self) -> u8 {
        self.0
    }
}
impl_FormatField!(<u8> Test);
fn main() {
    let t = Test(5);
    assert_eq!("5", t.format_type());
}
```
This has the benefit, that it will error at the impl macro if the trait bounds aren't satisfied, not at the method invocation.
### No Macro
Sometimes the impl_Impl macros might not be desired.
In that case it may be disabled with the `no_macro` option.
```rust
use abstract_impl::abstract_impl;
#[abstract_impl(no_macro)]
impl DebugToString for ToString where Self: std::fmt::Debug {
    fn to_string(&self) -> String {
        format!("{context:?}")
    }
}
// impl_DebugToString(());
// No impl_Impl macro generated
// Manually use it instead
#[derive(Debug)]
struct Test;
impl ToString for Test {
    fn to_string(&self) -> String {
        DebugToString::to_string::<Self>(self)
    }
}
```
### No Dummy
`abstract_impl` automatically generates a dummy implementation for the trait, to check that all items are implemented.
This may result in problems for traits that have super traits.
In that case it may be disabled with the `no_dummy` option.
```rust
use abstract_impl::abstract_impl;
#[abstract_impl(no_dummy)]
impl PartialUsingOrd for std::cmp::PartialOrd
where
    Self: Ord,
{
    fn partial_cmp(&self, rhs: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(rhs))
    }
}
```
If `no_dummy` wasn't used, you would get an error that Dummy doesn't implement Ord.
### Legacy Order
By default `impl Impl for Trait` is used.
Some people may prefer the previous order `impl Trait for Impl`.
```rust
use abstract_impl::abstract_impl;
#[abstract_impl(legacy_order)]
impl ToString for DebugToString where Self: std::fmt::Debug {
    fn to_string(&self) -> String {
        format!("{context:?}")
    }
}
```
## How it is made
After first trying to implement this functionality closer to CGP with an inherent impl block (on a type),
I switched to using modules, since inherent types are still **very** unstable (experimental).

The current implementation simply copies all trait (where clause) bounds to the trait items,
prepends a Context generic type (and all generics) and replaces Self/self with Context/context where it can.

The beginning example turns into:
```rust
mod DebugToString {
    // the actual impl (with context instead of self and prepended Context generic and where bounds)
    fn to_string<Context>(context: &Context) -> String where Context: std::fmt::Debug {
        format!("{context:?}")
    }
    // dummy impl to make shure all items are implemented
    struct Dummy;
    impl ToString for Dummy {
        fn to_string(&self) -> String {
            unimplemented!()
        }
    }
    // impl macro to give types a simple way of using it
    macro_rules! impl_DebugToString {
        ($t:ty) => {
            impl ToString for $t {
                fn to_string(&self) -> String {
                    DebugToString::to_string::<Self>(self)
                }
            }
        }
    }
}
```
There are some more edge cases this handles, like referencing associated types declared in the same trait,
associated types, that do not depend on Self (would get a type error otherwise), etc.

## Future Plans
- [ ] const item in impl (currently has no generics)
- [x] generics for trait and impl
- [ ] self in macros (expand inner first)
- [x] improved generic eliding for associated types (also check if generics were used)
- [ ] helper macros (like derive(UseType) and similar)
## Changelog
 - 0.1.0 Working version with basic documentation
 - 0.2.0 Reverse trait and name position (now impl Impl for Trait) and extended documentation
 - 0.2.1 Explanation on how it works, better errors and generics
 - 0.2.2 Better generic eliding (UseType pattern now kinda works)
## License
This code is MIT licensed.
