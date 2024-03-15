use proc_macro::TokenStream;
use quote::quote;

#[proc_macro_attribute]
pub fn tracked(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = proc_macro2::TokenStream::from(item);
    let res = quote! {
        #[::sefirot::luisa::prelude::tracked(crate = "::sefirot::luisa")]
        #item
    };
    res.into()
}

#[proc_macro]
pub fn track(input: TokenStream) -> TokenStream {
    let input = proc_macro2::TokenStream::from(input);
    let res = quote! {
        ::sefirot::luisa::prelude::tracked!(crate = "::sefirot::luisa" => #input)
    };
    res.into()
}
