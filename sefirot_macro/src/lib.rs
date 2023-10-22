use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned, ToTokens};
use syn::spanned::Spanned;
use syn::visit_mut::VisitMut;
use syn::*;

fn derive_structure_impl(input: DeriveInput) -> TokenStream {
    let Data::Struct(st) = input.data else {
        panic!("Structure can only be derived for structs");
    };

    let st_name = input.ident;
    let generics = input.generics;
    let vis = input.vis;
    let sf_path = quote! { ::sefirot::field::array::structure };
    let luisa_path = quote! { ::sefirot::luisa::lang::types };
    let mapped_st_name = Ident::new(&format!("{}Mapped", st_name), st_name.span());
    let where_clause = generics.where_clause;
    let generics = generics.params.into_iter().collect::<Vec<_>>();
    let generics_stripped = generics
        .iter()
        .map(|generic| match generic {
            GenericParam::Type(ty) => ty.ident.to_token_stream(),
            GenericParam::Lifetime(lt) => lt.lifetime.to_token_stream(),
            GenericParam::Const(c) => c.ident.to_token_stream(),
        })
        .collect::<Vec<_>>();
    let (fields, is_tuple) = match st.fields {
        Fields::Named(fields) => (fields.named, false),
        Fields::Unnamed(fields) => (fields.unnamed, true),
        Fields::Unit => panic!("Structure can only be derived for structs with fields"),
    };
    let field_types = fields.iter().map(|f| f.ty.clone()).collect::<Vec<_>>();

    let mapped_fields = fields
        .into_iter()
        .map(|mut f| {
            let span = f.ty.span();
            let ty = f.ty;
            f.ty = parse_quote_spanned! {span=>
                _Map::Result<#ty>
            };
            f
        })
        .collect::<Vec<_>>();

    let field_names = mapped_fields
        .iter()
        .enumerate()
        .map(|(i, f)| {
            f.ident
                .clone()
                .unwrap_or_else(|| Ident::new(&i.to_string(), f.span()))
        })
        .collect::<Vec<_>>();
    let selector_names = field_names
        .iter()
        .map(|f| Ident::new(&format!("{}_{}", st_name, f), f.span()))
        .collect::<Vec<_>>();

    let selectors = selector_names.iter().enumerate().map(|(i, sel)| {
        let fi = &field_names[i];
        let ft = &field_types[i];
        quote! {
            #[allow(non_camel_case_types)]
            #vis enum #sel {}
            impl<#(#generics),*> #sf_path::Selector<#st_name<#(#generics_stripped),*>> for #sel #where_clause {
                type Result = #ft;
                fn select_expr(structure: &#luisa_path::Expr<#st_name<#(#generics_stripped),*>>) -> Expr<Self::Result> {
                    structure.#fi.clone()
                }
                fn select_var(structure: &#luisa_path::Var<#st_name<#(#generics_stripped),*>>) -> Var<Self::Result> {
                    structure.#fi.clone()
                }

                fn select_ref(structure: &#st_name<#(#generics_stripped),*>) -> &#ft {
                    &structure.#fi
                }
                fn select_mut(structure: &mut #st_name<#(#generics_stripped),*>) -> &mut #ft {
                    &mut structure.#fi
                }
                fn select(structure: #st_name<#(#generics_stripped),*>) -> #ft {
                    structure.#fi
                }
            }
        }
    });

    let mapped_fields_decl = if is_tuple {
        quote!((#(#mapped_fields,)*))
    } else {
        quote!({#(#mapped_fields,)*})
    };

    let mapped_fields_apply = if is_tuple {
        quote! {
            #mapped_st_name(
                #(f.map::<#selector_names>(stringify!(#field_names)),)*
            )
        }
    } else {
        quote! {
            #mapped_st_name {
                #(#field_names: f.map::<#selector_names>(stringify!(#field_names)),)*
            }
        }
    };

    let bevy_derives = if cfg!(feature = "bevy") {
        quote! {
            #[derive(::sefirot::_bevy_ecs::prelude::Resource, ::sefirot::_bevy_ecs::prelude::Component)]
        }
    } else {
        quote!()
    };

    quote! {
        #bevy_derives
        #[derive(Debug, Clone, PartialEq, Eq)]
        #vis struct #mapped_st_name<#(#generics,)* _Map: #sf_path::Mapping> #where_clause #mapped_fields_decl
        const _: () = {
            #(#selectors)*
            impl<#(#generics),*> #sf_path::Structure for #st_name<#(#generics_stripped),*> #where_clause {
                type Map<M: #sf_path::Mapping> = #mapped_st_name<#(#generics_stripped,)* M>;
                fn apply<M: #sf_path::Mapping>(mut f: impl #sf_path::ValueMapping<Self, M = M>) -> Self::Map<M> {
                    #mapped_fields_apply
                }
            }
        };
    }
}

#[proc_macro_derive(Structure)]
pub fn derive_structure(tokens: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(tokens as DeriveInput);
    let tokens = derive_structure_impl(input);
    proc_macro::TokenStream::from(tokens)
}

struct RewriteIndexVisitor {
    needs_parens: bool,
}
impl VisitMut for RewriteIndexVisitor {
    fn visit_expr_mut(&mut self, expr: &mut Expr) {
        let span = expr.span();
        let np = self.needs_parens;
        self.needs_parens = true;
        match expr {
            Expr::Binary(node) => {
                self.needs_parens = false;
                self.visit_expr_mut(&mut node.left);
                self.needs_parens = false;
                self.visit_expr_mut(&mut node.right);
                return;
            }
            Expr::Index(node) => {
                let n_expr = &node.expr;
                let index = &node.index;
                if let Expr::Array(ExprArray { elems, .. }) = &**index {
                    if elems.len() == 1 {
                        let index = &elems[0];
                        let attrs = &node.attrs;
                        let span = span.resolved_at(Span::mixed_site());
                        if np {
                            *expr = parse_quote_spanned! {span=>
                                #(#attrs)*
                                (*(#index).__at(#n_expr))
                            };
                        } else {
                            *expr = parse_quote_spanned! {span=>
                                #(#attrs)*
                                *(#index).__at(#n_expr)
                            }
                        }
                    }
                }
            }
            Expr::Macro(expr) => {
                let path = &expr.mac.path;
                if path.leading_colon.is_none()
                    && path.segments.len() == 1
                    && path.segments[0].arguments.is_none()
                {
                    let ident = &path.segments[0].ident;
                    if *ident == "escape" {
                        return;
                    }
                }
            }
            _ => {}
        }
        visit_mut::visit_expr_mut(self, expr);
    }
}

fn track2_impl(mut ast: Expr) -> TokenStream {
    (RewriteIndexVisitor { needs_parens: true }).visit_expr_mut(&mut ast);

    quote!(::sefirot::luisa::prelude::track!(#ast))
}

#[proc_macro]
pub fn track(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = TokenStream::from(input);
    let input = quote!({ #input });
    let input = proc_macro::TokenStream::from(input);
    track2_impl(parse_macro_input!(input as Expr)).into()
}

#[proc_macro_attribute]
pub fn tracked(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let item = syn::parse_macro_input!(item as ItemFn);
    let body = &item.block;
    let body = proc_macro::TokenStream::from(quote!({ #body }));
    let body = track2_impl(parse_macro_input!(body as Expr));
    let attrs = &item.attrs;
    let sig = &item.sig;
    let vis = &item.vis;
    quote_spanned!(item.span()=> #(#attrs)* #vis #sig { #body }).into()
}

#[test]
fn test_track2() {
    let input = parse_quote! {
        {
            |el: &Element<Particles>, dt: Expr<f32>| {
                position[el] += velocity[el] * dt;
                let pos = position[el].cast_u32();

                display.write(pos, Vec4::splat(1.0));
            }
        }
    };
    let f = track2_impl(input);
    let file: File = parse_quote!(
        fn main() {
            #f
        }
    );
    panic!("{}", prettyplease::unparse(&file));
}

#[test]
fn test_derive() {
    let input = parse_quote! {
        struct S {
            a: u32,
            b: f32,
        }
    };
    let f = derive_structure_impl(input);
    let file: File = parse_quote!(#f);
    panic!("{}", prettyplease::unparse(&file));
}
