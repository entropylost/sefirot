use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;
use syn::*;

fn kernel_impl(init: bool, f: ItemFn) -> TokenStream {
    let bevy_luisa_path: Path = parse_quote!(::bevy_luisa);
    let luisa_path: Path = parse_quote!(::bevy_luisa::luisa);
    let span = f.span();
    let vis = f.vis;
    let mut sig = f.sig;
    let mut block = f.block;
    let attrs = f.attrs;
    let Stmt::Expr(Expr::Closure(kernel), None) = block.stmts.pop().unwrap() else {
        panic!("Kernel must have closure as last statement in body.");
    };
    let kernel_args = kernel
        .inputs
        .iter()
        .map(|pat| match pat {
            Pat::Type(PatType { ty, .. }) => {
                parse_quote!(<#ty as #luisa_path::runtime::KernelParameter>::Arg)
            }
            _ => panic!("Kernel arguments must be typed."),
        })
        .collect::<Vec<Type>>();
    let kernel_sig: Type = parse_quote!(fn(#(#kernel_args),*));
    let kernel_name = sig.ident;

    sig.ident = Ident::new(&format!("init_{}", kernel_name), kernel_name.span());
    if !sig.inputs.iter().any(|x| match x {
        FnArg::Receiver(_) => panic!("Kernel init function cannot have a self parameter."),
        FnArg::Typed(PatType { pat, .. }) => {
            if let Pat::Ident(PatIdent { ident, .. }) = &**pat {
                ident == "device"
            } else {
                false
            }
        }
    }) {
        sig.inputs
            .push(parse_quote!(device: ::bevy::prelude::Res<#bevy_luisa_path::LuisaDevice>));
        sig.inputs.push(parse_quote!(kernel_build_options: ::bevy::prelude::Res<#bevy_luisa_path::DefaultKernelBuildOptions>));
    }
    block.stmts.push(parse_quote! {
        #kernel_name.init(device.create_kernel_from_fn_with_name(&**kernel_build_options, stringify!(#kernel_name), #kernel));
    });

    let init_name = sig.ident.clone();

    let init = init.then(|| {
        quote! {
            #bevy_luisa_path::inventory::submit! {
                static _LAZY: #bevy_luisa_path::once_cell::sync::Lazy<
                    std::sync::Mutex<Option<Box<
                        dyn System<In = (), Out = ()>
                >>>> = #bevy_luisa_path::once_cell::sync::Lazy::new(||
                    std::sync::Mutex::new(
                        Some(
                            Box::new(
                                ::bevy::prelude::IntoSystem::into_system(#init_name)
                            )
                        )
                    )
                );
                #bevy_luisa_path::KernelRegistrationSystem(&_LAZY)
            }
        }
    });

    quote_spanned! {span=>
        #[allow(non_upper_case_globals)]
        #vis static #kernel_name: #bevy_luisa_path::KernelCell<#kernel_sig> = #bevy_luisa_path::KernelCell::default();
        #(#attrs)*
        #[#bevy_luisa_path::luisa::prelude::tracked]
        #sig #block
        #init
    }
}

#[proc_macro_attribute]
pub fn kernel(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let init = match &*attr.to_string() {
        "" => true,
        "noinit" => false,
        _ => panic!("Invalid attribute"),
    };
    let f = parse_macro_input!(item as ItemFn);
    kernel_impl(init, f).into()
}

#[test]
fn test_kernel() {
    let f = parse_quote! {
        fn clear_display_kernel() {
            |display: Tex2dVar<Vec4<f32>>, clear_color: Expr<Vec4<f32>>| {
                display.write(dispatch_id().xy(), clear_color);
            }
        }
    };
    let f = kernel_impl(true, f);
    let file: File = parse_quote!(#f);
    panic!("{}", prettyplease::unparse(&file));
}
