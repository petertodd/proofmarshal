use std::convert::TryFrom;

use proc_macro2::{TokenStream, Span};
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{parse_macro_input, parse_quote, Data, DeriveInput, Fields, GenericParam, TypeParam, Generics, Index};
use syn::parse_quote::ParseQuote;

pub fn derive_verbatim(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    // Parse the input tokens into a syntax tree.
    let input = parse_macro_input!(input as DeriveInput);

    // Used in the quasi-quotation below as `#name`.
    let name = input.ident;
    let vis = input.vis;

    let errname = syn::Ident::new(&format!("{}DecodeError", name), Span::call_site());
    let ptr_ty  = syn::Ident::new("__P", Span::call_site());

    let generics = add_trait_bounds(input.generics, &ptr_ty);
    let mut generics2 = generics.clone();

    let (_, ty_generics, where_clause) = generics.split_for_impl();


    generics2.params.push(parse_quote!(#ptr_ty: ::proofmarshal::ptr::Ptr));
    let (impl_generics, _, _) = generics2.split_for_impl();

    let encode = verbatim_encode(&input.data, &ptr_ty);
    let decode = verbatim_decode(&input.data, &errname, &ptr_ty);
    let verbatim_len = verbatim_len(&input.data, &ptr_ty);
    let nonzero_niche = verbatim_nonzero_niche(&input.data, &ptr_ty);


    let expanded = quote! {
        #[derive(Debug,PartialEq,Eq)]
        #vis struct #errname(&'static str);

        // The generated impl.
        #[automatically_derived]
        impl #impl_generics ::proofmarshal::verbatim::Verbatim<#ptr_ty> for #name #ty_generics #where_clause {
            type Error = #errname;

            const LEN: usize = #verbatim_len;
            const NONZERO_NICHE: bool = #nonzero_niche;

            #[inline]
            fn encode<__W: ::std::io::Write>(&self, __dst: __W,
                                             __ptr_encoder: &mut impl ::proofmarshal::verbatim::PtrEncode<#ptr_ty>)
                -> Result<__W, ::std::io::Error>
            {
                #encode
            }

            #[inline]
            fn decode(__src: &[u8],
                      __ptr_decoder: &mut impl ::proofmarshal::verbatim::PtrDecode<#ptr_ty>)
                -> Result<Self, Self::Error>
            {
                #decode
            }
        }
    };

    // Hand the output tokens back to the compiler.
    proc_macro::TokenStream::from(expanded)
}

fn add_trait_bounds(mut generics: Generics, ptr_ty: &syn::Ident) -> Generics {
    for param in &mut generics.params {
        if let GenericParam::Type(ref mut type_param) = *param {
            type_param.bounds.push(parse_quote!(::proofmarshal::verbatim::Verbatim<#ptr_ty>));
        }
    }
    generics
}


fn len_for_fields(fields: &syn::Fields, ptr_ty: &syn::Ident) -> TokenStream {
    let per_field = |field: &syn::Field| {
            let ty = &field.ty;
            quote_spanned! {field.span()=>
                <#ty as ::proofmarshal::verbatim::Verbatim<#ptr_ty>>::LEN
            }
    };

    match fields {
        Fields::Named(named) => {
            let recurse = named.named.iter().map(per_field);
            quote! {
                0 #(+ #recurse )*
            }
        }
        Fields::Unnamed(unnamed) => {
            let recurse = unnamed.unnamed.iter().map(per_field);
            quote! {
                0 #(+ #recurse )*
            }
        }
        Fields::Unit => {
            quote! {
                0
            }
        }
    }
}

fn verbatim_len(data: &Data, ptr_ty: &syn::Ident) -> TokenStream {
    match data {
        Data::Struct(data) => len_for_fields(&data.fields, ptr_ty),
        Data::Enum(data) => {
            let variants: Vec<&syn::Variant> = data.variants.iter().collect();

            assert!(variants.len() < u8::max_value() as usize);

            let discriminant: usize = match variants.len() {
                0 => 0,
                x => 1,
            };

            let recurse = variants.iter().map(|variant| {
                let variant_len = len_for_fields(&variant.fields, ptr_ty);
                quote! {
                    let __r = __max(__r, #variant_len);
                }
            });

            quote! { {
                const fn __max(__lhs: usize, __rhs: usize) -> usize {
                    (((__lhs >  __rhs) as usize) * __lhs) +
                    (((__rhs >= __lhs) as usize) * __rhs)
                };

                let __r = 0;

                #( #recurse )*

                #discriminant + __r
            }}
        },
        Data::Union(_) => unimplemented!(),
    }
}

fn verbatim_nonzero_niche(data: &Data, ptr_ty: &syn::Ident) -> TokenStream {
    match *data {
        Data::Struct(ref data) => {
            match data.fields {
                Fields::Named(ref fields) => {
                    let recurse = fields.named.iter().map(|f| {
                        let ty = &f.ty;
                        quote_spanned! {f.span()=>
                            <#ty as ::proofmarshal::verbatim::Verbatim<#ptr_ty>>::NONZERO_NICHE
                        }
                    });
                    quote! {
                        false #(| #recurse)*
                    }
                }
                Fields::Unnamed(ref fields) => {
                    let recurse = fields.unnamed.iter().map(|f| {
                        let ty = &f.ty;
                        quote_spanned! {f.span()=>
                            <#ty as ::proofmarshal::verbatim::Verbatim<#ptr_ty>>::NONZERO_NICHE
                        }
                    });
                    quote! {
                        false #(| #recurse)*
                    }
                }
                Fields::Unit => {
                    quote! {
                        false
                    }
                }
            }
        }
        Data::Enum(_) => {
            quote! {
                false
            }
        }
        Data::Union(_) => unimplemented!(),
    }
}

fn bind_unnamed_fields(fields: &syn::FieldsUnnamed) -> Vec<syn::Ident> {
    fields.unnamed.iter().enumerate().map(|(i,f)| {
        syn::Ident::new(&format!("__self_{}", i), Span::call_site())
    }).collect()
}

fn bind_named_fields(fields: &syn::FieldsNamed) -> Vec<TokenStream> {
    fields.named.iter().enumerate().map(|(i,f)| {
        let name = &f.ident;
        let bind = syn::Ident::new(&format!("__self_{}", i), Span::call_site());
        quote! { #name: #bind }
    }).collect()
}


/// Binds fields then does something with them.
fn bind_fields_then(fields: &syn::Fields,
                    mut f: impl FnMut(&mut dyn Iterator<Item=(syn::Ident, &syn::Field)>) -> TokenStream)
    -> (TokenStream, TokenStream)
{
    match fields {
        syn::Fields::Named(named) => {
            let binder = named.named.iter().enumerate().map(|(i,field)| {
                let name = &field.ident;
                let bind = syn::Ident::new(&format!("__self_{}", i), Span::call_site());

                quote! {
                    #name: #bind
                }
            });

            let mut doer = named.named.iter().enumerate().map(|(i,field)| {
                let bind = syn::Ident::new(&format!("__self_{}", i), Span::call_site());

                (bind, field)
            });

            (quote! { { #( #binder ),* } },
             f(&mut doer))

        },
        syn::Fields::Unnamed(unnamed) => {
            let binder = unnamed.unnamed.iter().enumerate().map(|(i,field)| {
                syn::Ident::new(&format!("__self_{}", i), Span::call_site())
            });

            let mut doer = unnamed.unnamed.iter().enumerate().map(|(i,field)| {
                let bind = syn::Ident::new(&format!("__self_{}", i), Span::call_site());

                (bind, field)
            });

            (quote! { ( #( #binder ),* ) },
             f(&mut doer))
        },
        syn::Fields::Unit => {
            (quote!{}, f(&mut std::iter::empty()))
        }
    }
}

fn verbatim_encode(data: &Data, ptr_ty: &syn::Ident) -> TokenStream {
    let verbatim_encode_fields = |fields: &mut dyn Iterator<Item=(syn::Ident, &syn::Field)>| {
        let per_field = fields.map(|(binding, field)| {
            let ty = &field.ty;
            quote! {
                let __dst = <#ty as ::proofmarshal::verbatim::Verbatim<#ptr_ty>>::encode(#binding, __dst, __ptr_encoder)?;
            }
        });

        quote!{
            #( #per_field )*
        }
    };

    match *data {
        Data::Struct(ref data) => {
            let (binder, doer) = bind_fields_then(&data.fields, verbatim_encode_fields);


            quote! {
                let Self #binder = self;

                #doer

                Ok(__dst)
            }
        }
        Data::Enum(ref data) => {
            let per_variant = data.variants.iter().enumerate().map(|(idx, variant)| {
                let variant_name = &variant.ident;

                let discriminant_expr: syn::Expr = match &variant.discriminant {
                    None => {
                        let idx = u8::try_from(idx).expect("discriminant out of range");
                        parse_quote!{ #idx }
                    },
                    Some(discriminant) => unimplemented!(),
                };

                let (binder, doer) = bind_fields_then(&variant.fields, verbatim_encode_fields);

                quote! {
                    Self::#variant_name #binder => {
                        let mut __dst = __dst;
                        __dst.write_all(&[#discriminant_expr])?;

                        #doer

                        Ok(__dst)
                }}
            });

            quote! {
                match self {
                    #( #per_variant )*
                }
            }
        }
        Data::Union(_) => unimplemented!("unions are not implemented"),
    }
}

fn verbatim_decode(data: &Data, errname: &syn::Ident, ptr_ty: &syn::Ident) -> TokenStream {
    let verbatim_decode_fields = |fields: &mut dyn Iterator<Item=(syn::Ident, &syn::Field)>| {
        let per_field = fields.map(|(binding, field)| {
            let ty = &field.ty;
            quote! {
                let (__field_buf, __src) = __src.split_at(<#ty as ::proofmarshal::verbatim::Verbatim<#ptr_ty>>::LEN);
                let #binding = <#ty as ::proofmarshal::verbatim::Verbatim<#ptr_ty>>::decode(__field_buf, __ptr_decoder)
                                                                      .map_err(|_| #errname(::core::any::type_name::<#ty>()))?;
            }
        });

        quote!{
            #( #per_field )*
        }
    };

    match *data {
        Data::Struct(ref data) => {
            let (binder, doer) = bind_fields_then(&data.fields, verbatim_decode_fields);

            quote! {
                #doer

                Ok(Self #binder)
            }
        }
        Data::Enum(ref data) => {
            let per_variant = data.variants.iter().enumerate().map(|(idx, variant)| {
                let variant_name = &variant.ident;

                let discriminant_expr: syn::Expr = match &variant.discriminant {
                    None => {
                        let idx = u8::try_from(idx).expect("discriminant out of range");
                        parse_quote!{ #idx }
                    },
                    Some(discriminant) => unimplemented!("custom discriminant not yet supported"),
                };

                let (binder, doer) = bind_fields_then(&variant.fields, verbatim_decode_fields);

                quote! {
                    #discriminant_expr => {
                        #doer

                        __padding = __src;

                        Self::#variant_name #binder
                }}
            });

            quote! {
                assert_eq!(__src.len(), <Self as ::proofmarshal::verbatim::Verbatim<__P>>::LEN);
                if __src.len() == 0 {
                    panic!("{} is uninhabited", ::core::any::type_name::<Self>())
                }

                let __discriminant = __src[0];
                let __src = &__src[1..];
                let __padding;
                let __r = match __discriminant {
                    #( #per_variant )*
                    _ => { return Err(#errname(&"invalid discriminant")); },
                };

                // Verify padding is zeroed
                for &__b in __padding {
                    if __b != 0 {
                        return Err(#errname(&"nonzero padding"));
                    }
                }

                Ok(__r)
            }
        }
        Data::Union(_) => unimplemented!("unions are not implemented"),
    }
}
