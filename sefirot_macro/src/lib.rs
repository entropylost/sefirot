use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::spanned::Spanned;
use syn::*;

fn derive_structure_impl(input: DeriveInput) -> TokenStream {
    let Data::Struct(st) = input.data else {
        panic!("Structure can only be derived for structs");
    };

    let st_name = input.ident;
    let generics = input.generics;
    let sf_path = quote! { ::sefirot::accessor::array::structure };
    let luisa_path = quote! { ::sefirot::prelude::luisa::lang::types };
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
            pub enum #sel {}
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

    quote! {
        const _: () = {
            #(#selectors)*
            pub struct #mapped_st_name<#(#generics,)* _Map: #sf_path::Mapping> #where_clause #mapped_fields_decl
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

#[test]
fn test_kernel() {
    let input = parse_quote! {
        struct S {
            a: u32,
            b: f32,
        }
    };
    let f = derive_structure_impl(input);
    // panic!("{}", f);
    let file: File = parse_quote!(#f);
    panic!("{}", prettyplease::unparse(&file));
}
