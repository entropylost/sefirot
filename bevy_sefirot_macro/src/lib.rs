use proc_macro2::TokenStream;
use quote::quote_spanned;
use syn::spanned::Spanned;
use syn::*;

fn kernel_impl(f: ItemFn, init_vis: Visibility) -> TokenStream {
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
    let (index_type, kernel_sig, domain_args_sig) = {
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
        let GenericArgument::Type(index_type) = &args[0] else {
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
        (index_type.clone(), kernel_sig.clone(), domain_args_sig)
    };
    let kernel_name = sig.ident;

    let mut last_stmt = block.stmts.pop().unwrap();
    if let Stmt::Expr(Expr::Call(ExprCall { func, .. }), None) = &mut last_stmt {
        if let Expr::Path(ExprPath {
            path:
                Path {
                    leading_colon: None,
                    segments,
                },
            qself: None,
            ..
        }) = &mut **func
        {
            if segments.len() == 2
                && ["build", "build_with_options"].contains(&&*segments[1].ident.to_string())
                && segments[0].ident == "Kernel"
                && segments[0].arguments == PathArguments::None
            {
                segments[0].arguments = PathArguments::AngleBracketed(
                    parse_quote!(::<#index_type, #kernel_sig, #domain_args_sig>),
                );
                last_stmt = Stmt::Expr(
                    parse_quote! {
                        #last_stmt.with_name(stringify!(#kernel_name))
                    },
                    None,
                );
            }
        }
    }
    block.stmts.push(parse_quote! {
        #kernel_name.init(#last_stmt);
    });

    sig.ident = Ident::new(&format!("init_{}", kernel_name), kernel_name.span());

    quote_spanned! {span=>
        #[allow(non_upper_case_globals)]
        #vis static #kernel_name: #bevy_sefirot_path::KernelCell<#index_type, #kernel_sig, #domain_args_sig> =
            #bevy_sefirot_path::KernelCell::default();
        #(#attrs)*
        #[forbid(dead_code)]
        #[tracked]
        #init_vis #sig #block
    }
}

/// Initializes a function returning a kernel during `PostStartup`.
/// To use most kernel functions, use the `tracked` attribute or `track` macro.
#[proc_macro_attribute]
pub fn kernel(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let f = parse_macro_input!(item as ItemFn);
    let init_vis = parse_macro_input!(attr as Visibility);
    kernel_impl(f, init_vis).into()
}

#[test]
fn test_kernel() {
    let f = parse_quote! {
        fn clear_display_kernel(particles: Res<Emanation<Particles>>, device: LuisaDevice, domain: Res<ArrayIndex>) -> Kernel<Particles, fn(Tex2d<Vec4<f32>>, Vec4<f32>)> {
            Kernel::build(domain, |el, display, clear_color| {
                display.write(dispatch_id().xy(), clear_color);
            })
        }
    };
    let f = kernel_impl(f, Visibility::Inherited);
    let file: File = parse_quote!(#f);
    panic!("{}", prettyplease::unparse(&file));
}
