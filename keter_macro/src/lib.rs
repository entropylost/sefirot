use proc_macro2::TokenStream;
use quote::quote;

#[proc_macro_attribute]
pub fn tracked(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let item = TokenStream::from(item);
    let res = quote! {
        #[::keter::_luisa::prelude::tracked(crate = "::keter::_luisa")]
        #item
    };
    res.into()
}

#[proc_macro]
pub fn track(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = TokenStream::from(input);
    let res = quote! {
        ::keter::_luisa::prelude::track!(crate = "::keter::_luisa" => #input)
    };
    res.into()
}
