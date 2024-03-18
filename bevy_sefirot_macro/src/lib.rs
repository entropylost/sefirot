use proc_macro2::TokenStream;
use quote::quote_spanned;
use syn::parse::Parser;
use syn::spanned::Spanned;
use syn::token::Paren;
use syn::*;

// TODO: Add option to disable fast math.
// Also offer "run" to generate the running system.
// Use init(pub) to mean making the init public.
// See https://docs.rs/syn/latest/syn/meta/fn.parser.html
fn kernel_impl(f: ItemFn, attributes: Attributes) -> TokenStream {
    let init_vis = attributes.init_vis;
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
    let (kernel_sig, domain_args_sig) = {
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
        if args.len() != 1 && args.len() != 2 {
            panic!("Function must return a `Kernel`.");
        }
        let GenericArgument::Type(kernel_sig) = &args[0] else {
            panic!("Function must return a `Kernel`.");
        };
        let domain_args_sig = if args.len() == 2 {
            match &args[1] {
                GenericArgument::Type(x) => x.clone(),
                _ => panic!("Function must return a `Kernel`."),
            }
        } else {
            parse_quote!(())
        };
        (kernel_sig.clone(), domain_args_sig)
    };
    let kernel_name = sig.ident;

    let mut last_stmt = block.stmts.pop().unwrap();
    if let Stmt::Expr(Expr::Call(ExprCall { func, args, .. }), None) = &mut last_stmt {
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
            {
                if segments[0].arguments == PathArguments::None {
                    segments[0].arguments = PathArguments::AngleBracketed(
                        parse_quote!(::<#kernel_sig, #domain_args_sig>),
                    );
                }
                let closure_index = args.len() - 1;
                let ac = &mut args[closure_index];
                *ac = parse_quote! {
                    ::sefirot::prelude::track!(#ac)
                };
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

    let run_impl = attributes.run.map(|run| {
        let name = run.name.map_or_else(
            || {
                let kn = kernel_name.to_string();
                Ident::new(
                    &if kn.ends_with("_kernel") {
                        kn[0..kn.len() - 7].to_string()
                    } else {
                        format!("run_{}", kn)
                    },
                    kernel_name.span(),
                )
            },
            |name| Ident::new(&name.value(), name.span()),
        );
        let vis = run.vis;
        quote_spanned! {span=>
            #vis fn #name() -> impl ::sefirot::graph::AsNodes<'static> {
                #kernel_name.dispatch()
            }
        }
    });

    quote_spanned! {span=>
        #[allow(non_upper_case_globals)]
        #vis static #kernel_name: #bevy_sefirot_path::KernelCell<#kernel_sig, #domain_args_sig> =
            #bevy_sefirot_path::KernelCell::default();
        #(#attrs)*
        #[forbid(dead_code)]
        #init_vis #sig #block
        #run_impl
    }
}
struct Run {
    name: Option<LitStr>,
    vis: Visibility,
}

struct Attributes {
    init_vis: Visibility,
    run: Option<Run>,
    // TODO: Use
    fast_math: Option<LitBool>,
}

fn parse_attrs(attr: TokenStream) -> std::result::Result<Attributes, TokenStream> {
    let mut init_vis = Visibility::Inherited;
    let mut run = None;
    let mut fast_math = None;
    let parser = meta::parser(|meta| {
        if meta.path.is_ident("init") {
            let content;
            parenthesized!(content in meta.input);
            init_vis = content.parse()?;
        } else if meta.path.is_ident("run") {
            let mut this_run = Run {
                name: None,
                vis: Visibility::Inherited,
            };
            if meta.input.peek(Paren) {
                let content;
                parenthesized!(content in meta.input);
                this_run.vis = content.parse()?;
            }
            if let Ok(value) = meta.value() {
                this_run.name = Some(value.parse()?);
            }
            run = Some(this_run);
        } else if meta.path.is_ident("fast_math") {
            let value = meta.value()?;
            fast_math = Some(value.parse()?);
        } else {
            return Err(meta.error("unsupported attribute"));
        }
        Ok(())
    });
    if let Err(err) = parser.parse2(attr) {
        return Err(err.to_compile_error());
    }
    Ok(Attributes {
        init_vis,
        run,
        fast_math,
    })
}

/// Initializes a function returning a kernel during `PostStartup`.
/// This automatically adds a [`sefirot::track!`] around the closure passed to [`Kernel::build`], if it exists.
#[proc_macro_attribute]
pub fn kernel(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let f = parse_macro_input!(item as ItemFn);
    let attrs = match parse_attrs(attr.into()) {
        Ok(attrs) => attrs,
        Err(err) => {
            return err.into();
        }
    };

    kernel_impl(f, attrs).into()
}

#[test]
fn test_kernel() {
    use quote::quote;
    let f = parse_quote! {
        fn clear_display_kernel(particles: Res<Emanation<Particles>>, device: LuisaDevice, domain: Res<ArrayIndex>) -> Kernel<fn(Tex2d<Vec4<f32>>, Vec4<f32>)> {
            Kernel::build(&device, &domain, |el, display, clear_color| {
                display.write(dispatch_id().xy(), clear_color);
            })
        }
    };
    let f = kernel_impl(
        f,
        parse_attrs(quote!(
            init(pub), run(pub) = "foo", fast_math = true
        ))
        .unwrap(),
    );
    let file: File = parse_quote!(#f);
    panic!("{}", prettyplease::unparse(&file));
}
