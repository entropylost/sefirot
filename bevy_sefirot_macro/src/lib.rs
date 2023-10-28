use proc_macro2::TokenStream;
use quote::{quote, quote_spanned, ToTokens};
use syn::spanned::Spanned;
use syn::*;

fn init_kernel_impl(f: ItemFn) -> TokenStream {
    let bevy_luisa_path: Path = parse_quote!(::bevy_sefirot::prelude::bevy_luisa);
    let bevy_sefirot_path: Path = parse_quote!(::bevy_sefirot);
    let span = f.span();
    let vis = f.vis;
    let mut sig = f.sig;
    let mut block = f.block;
    let attrs = f.attrs;
    let ReturnType::Type(_, kernel_type) = std::mem::replace(&mut sig.output, ReturnType::Default)
    else {
        panic!("Function must return a `Kernel`.");
    };
    let (emanation_ty, kernel_sig, domain_args_sig) = {
        let Type::Path(path) = &*kernel_type else {
            panic!("Function must return a `Kernel`.");
        };
        let last_path = path.path.segments.last().unwrap();
        if last_path.ident != "Kernel" {
            panic!("Function must return a `Kernel`.");
        }
        let PathArguments::AngleBracketed(AngleBracketedGenericArguments { args, .. }) =
            &last_path.arguments
        else {
            panic!("Function must return a `Kernel`.");
        };
        if args.len() != 2 && args.len() != 3 {
            panic!("Function must return a `Kernel`.");
        }
        let GenericArgument::Type(emanation_ty) = &args[0] else {
            panic!("Function must return a `Kernel`.");
        };
        let GenericArgument::Type(kernel_sig) = &args[1] else {
            panic!("Function must return a `Kernel`.");
        };
        let domain_args_sig = if args.len() == 3 {
            match &args[2] {
                GenericArgument::Type(x) => x.clone(),
                _ => panic!("Function must return a `Kernel`."),
            }
        } else {
            parse_quote!(())
        };
        (emanation_ty.clone(), kernel_sig.clone(), domain_args_sig)
    };
    let kernel_name = sig.ident;

    let mut last_stmt = block.stmts.pop().unwrap();
    if let Stmt::Expr(
        Expr::MethodCall(ExprMethodCall {
            method, turbofish, ..
        }),
        _,
    ) = &mut last_stmt
    {
        if method == "build_kernel" && turbofish.is_none() {
            *turbofish = Some(parse_quote!(::<#kernel_sig, #domain_args_sig>));
            *method = Ident::new("build_kernel_with_domain_args", method.span());
        }
    }
    block.stmts.push(parse_quote! {
        #kernel_name.init(#last_stmt);
    });

    sig.ident = Ident::new(&format!("init_{}", kernel_name), kernel_name.span());
    let init_name = sig.ident.clone();

    let init = quote! {
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
    };

    quote_spanned! {span=>
        #[allow(non_upper_case_globals)]
        #vis static #kernel_name: #bevy_sefirot_path::KernelCell<#emanation_ty, #kernel_sig, #domain_args_sig> =
            #bevy_sefirot_path::KernelCell::default();
        #(#attrs)*
        #sig #block
        #init
    }
}

/// Initializes a function returning a kernel during `PostStartup`.
/// To use most kernel functions, use the `tracked` attribute or `track` macro.
#[proc_macro_attribute]
pub fn init_kernel(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let f = parse_macro_input!(item as ItemFn);
    init_kernel_impl(f).into()
}

#[test]
fn test_kernel() {
    let f = parse_quote! {
        fn clear_display_kernel(particles: Res<Emanation<Particles>>, device: LuisaDevice, domain: Res<ArrayIndex>) -> Kernel<Particles, fn(Tex2d<Vec4<f32>>, Vec4<f32>)> {
            particles.build_kernel(domain, |el, display, clear_color| {
                display.write(dispatch_id().xy(), clear_color);
            })
        }
    };
    let f = init_kernel_impl(f);
    let file: File = parse_quote!(#f);
    panic!("{}", prettyplease::unparse(&file));
}
