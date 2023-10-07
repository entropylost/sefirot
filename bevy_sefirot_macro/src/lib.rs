use proc_macro2::TokenStream;
use quote::{quote, quote_spanned, ToTokens};
use syn::spanned::Spanned;
use syn::*;

fn kernel_impl(init: bool, f: ItemFn) -> TokenStream {
    let bevy_luisa_path: Path = parse_quote!(::bevy_sefirot::prelude::bevy_luisa);
    let bevy_sefirot_path: Path = parse_quote!(::bevy_sefirot);
    let luisa_path: Path = parse_quote!(::bevy_sefirot::prelude::luisa);
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
        .skip(1)
        .map(|pat| match pat {
            Pat::Type(PatType { ty, .. }) => {
                parse_quote!(<#ty as #luisa_path::runtime::KernelParameter>::Arg)
            }
            _ => panic!("Kernel arguments must be typed."),
        })
        .collect::<Vec<Type>>();
    let emanation_ty = {
        let ty = match &kernel.inputs[0] {
            Pat::Type(PatType { ty, .. }) => ty,
            _ => panic!("Kernel must have an &Element as its first argument"),
        };
        let Type::Reference(element) = &**ty else {
            panic!("Kernel must have an &Element as its first argument");
        };
        let Type::Path(TypePath { path, .. }) = &*element.elem else {
            panic!("Kernel must have an &Element as its first argument");
        };
        let last_path = path.segments.last().unwrap();
        if last_path.ident != "Element" {
            panic!("Kernel must have an &Element as its first argument");
        }
        let PathArguments::AngleBracketed(AngleBracketedGenericArguments { args, .. }) =
            &last_path.arguments
        else {
            panic!("Kernel must have an &Element as its first argument");
        };
        if args.len() != 1 {
            panic!("Kernel must have an &Element as its first argument");
        }
        let GenericArgument::Type(ty) = &args[0] else {
            panic!("Kernel must have an &Element as its first argument");
        };
        ty.clone()
    };
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
            .push(parse_quote!(device: #bevy_luisa_path::LuisaDevice));
    }
    let emanation = sig
        .inputs
        .iter()
        .find_map(|x| match x {
            FnArg::Typed(PatType { pat, ty, .. }) => {
                let Type::Path(TypePath { path, .. }) = &**ty else {
                    return None;
                };
                let last_path = path.segments.last()?;
                if last_path.ident != "Emanation" {
                    return None;
                }
                let PathArguments::AngleBracketed(AngleBracketedGenericArguments { args, .. }) =
                    &last_path.arguments
                else {
                    return None;
                };
                if args.len() != 1 {
                    return None;
                }
                let GenericArgument::Type(ty) = &args[0] else {
                    return None;
                };
                if emanation_ty.to_token_stream().to_string() != ty.to_token_stream().to_string() {
                    return None;
                }
                let Pat::Ident(PatIdent { ident, .. }) = &**pat else {
                    return None;
                };
                Some(ident.clone())
            }
            _ => unreachable!(),
        })
        .unwrap_or_else(|| {
            sig.inputs.push(
                parse_quote!(_emanation: #bevy_sefirot_path::prelude::Emanation<#emanation_ty>),
            );
            parse_quote!(_emanation)
        });
    block.stmts.push(parse_quote! {
        #kernel_name.init(#emanation.build_kernel_with_options(&device.device, &device.kernel_build_options, #bevy_sefirot_path::prelude::tracked!(#kernel)));
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
        #vis static #kernel_name: #bevy_sefirot_path::KernelCell<#emanation_ty, #kernel_sig> = #bevy_sefirot_path::KernelCell::default();
        #(#attrs)*
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
            |el: &Element<Particles>, display: Tex2dVar<Vec4<f32>>, clear_color: Expr<Vec4<f32>>| {
                display.write(dispatch_id().xy(), clear_color);
            }
        }
    };
    let f = kernel_impl(true, f);
    let file: File = parse_quote!(#f);
    panic!("{}", prettyplease::unparse(&file));
}
