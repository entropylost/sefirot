use std::collections::HashMap;

use proc_macro2::TokenStream;
use quote::quote_spanned;
use syn::parse::Parser;
use syn::spanned::Spanned;
use syn::token::Paren;
use syn::*;

fn kernel_impl(f: ItemFn, attributes: Attributes) -> TokenStream {
    let init_vis = attributes.init_vis;
    let init_name = attributes.init_name;
    let bevy_sefirot_path: Path = parse_quote!(::bevy_sefirot);
    let span = f.span();
    let build_options = attributes.build_options.0;
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
                && ["build", "build_named", "build_with_options"]
                    .contains(&&*segments[1].ident.to_string())
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
                if !build_options.is_empty() || segments[1].ident != "build_named" {
                    assert!(segments[1].ident == "build");
                    assert!(args.len() == 2);
                    let build_options = build_options.into_iter().collect::<Vec<_>>();
                    let keys = build_options.iter().map(|(k, _)| k);
                    let values = build_options.iter().map(|(_, v)| v);
                    let kernel_build_options: Expr = parse_quote! {
                        ::sefirot::luisa::runtime::KernelBuildOptions {
                            #(#keys: #values,)*
                            ..::sefirot::luisa::runtime::KernelBuildOptions {
                                name: Some(stringify!(#kernel_name).to_string()),
                                ..::sefirot::kernel::default_kernel_build_options()
                            }
                        }
                    };
                    args.insert(0, kernel_build_options);
                    segments[1].ident = Ident::new("build_with_options", segments[1].ident.span());
                }
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

    sig.ident = init_name
        .unwrap_or_else(|| Ident::new(&format!("init_{}", kernel_name), kernel_name.span()));

    let run_impl = attributes.run.map(|run| {
        let name = run.name.unwrap_or_else(|| {
            let kn = kernel_name.to_string();
            Ident::new(
                &if kn.ends_with("_kernel") {
                    kn[0..kn.len() - 7].to_string()
                } else {
                    format!("run_{}", kn)
                },
                kernel_name.span(),
            )
        });
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
    name: Option<Ident>,
    vis: Visibility,
}

struct BuildOptions(HashMap<Ident, Expr>);
impl BuildOptions {
    fn register(&mut self, key: Ident, value: Expr) -> Result<()> {
        let allowed_keys = [
            "enable_debug_info",
            "enable_optimization",
            "async_compile",
            "enable_cache",
            "enable_fast_math",
            "max_registers",
            "time_trace",
            "name",
            "native_include",
        ];
        if !allowed_keys.contains(&&*key.to_string()) {
            return Err(Error::new(
                key.span(),
                format!("unsupported build option: {}", key),
            ));
        }
        self.0.insert(key, value);
        Ok(())
    }
}

struct Attributes {
    init_vis: Visibility,
    init_name: Option<Ident>,
    run: Option<Run>,
    build_options: BuildOptions,
}

fn parse_attrs(attr: TokenStream) -> std::result::Result<Attributes, TokenStream> {
    let mut init_vis = Visibility::Inherited;
    let mut init_name = None;
    let mut run = None;
    let mut build_options = BuildOptions(HashMap::new());
    let parser = meta::parser(|meta| {
        if meta.path.is_ident("init") {
            let content;
            if meta.input.peek(Paren) {
                parenthesized!(content in meta.input);
                init_vis = content.parse()?;
            }
            if let Ok(value) = meta.value() {
                init_name = Some(value.parse()?);
            }
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
        } else {
            let key = meta
                .path
                .get_ident()
                .ok_or_else(|| Error::new(meta.path.span(), "expected identifier"))?;
            let value = meta.value()?;
            build_options.register(key.clone(), value.parse()?)?;
        }
        Ok(())
    });
    if let Err(err) = parser.parse2(attr) {
        return Err(err.to_compile_error());
    }
    Ok(Attributes {
        init_vis,
        init_name,
        run,
        build_options,
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
        pub fn compute_offset_kernel(grid: Res<Grid>) -> Kernel<fn()> {
            Kernel::build(&grid.domain, &|index| {
                let count = grid.count.read(*index);
                grid.offset
                    .write(*index, grid.next_block.atomic_ref(0).fetch_add(count));
                grid.count.write(*index, 0);
            })
        }
    };
    let f = kernel_impl(
        f,
        parse_attrs(quote!(
            init(pub)
        ))
        .unwrap(),
    );
    let file: File = parse_quote!(#f);
    panic!("{}", prettyplease::unparse(&file));
}
