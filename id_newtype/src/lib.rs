use quote::{format_ident, quote, ToTokens};
use syn::{Data, DeriveInput, Fields, parse_macro_input, Type};

#[proc_macro_derive(UniqueId)]
pub fn derive_unique_id(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let ident = input.ident;

    let Data::Struct(struct_) = input.data else { panic!("UniqueId can only be derived for structs."); };
    let fields = match &struct_.fields {
        Fields::Named(fields) => &fields.named,
        Fields::Unnamed(fields) => &fields.unnamed,
        Fields::Unit => panic!("Unit types cannot be counters.")
    };
    assert_eq!(fields.len(), 1, "Struct may only have one field.");
    let Type::Path(path) = &fields[0].ty else {
        panic!("Structs field must be an unsigned integer type.");
    };
    let name = format!("{}", path.to_token_stream());
    assert!(["u8", "u16", "u32", "u64", "usize"].contains(&&*name), "Structs field must be an unsigned integer type.");
    let atomic_name = format_ident!("AtomicU{}", &name[1..]);

    let create = quote! {
        NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    };
    let args = match &fields[0].ident {
        Some(ident) => quote!({ #ident: #create }),
        None => quote!((#create))
    };

    let out = quote! {
        const _: () = {
            static NEXT_ID: std::sync::atomic::#atomic_name = std::sync::atomic::#atomic_name::new(0);
            impl #ident {
                pub fn unique() -> Self {
                    Self #args
                }
            }
        };
    };
    out.into()
}
