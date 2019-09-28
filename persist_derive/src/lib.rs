extern crate proc_macro;

use proc_macro2::{TokenStream, Span};
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{parse_macro_input, parse_quote, Data, DeriveInput, Fields, GenericParam, Generics, Index};

fn validate_repr(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path.is_ident("repr")
    })
}

#[proc_macro_derive(Persist)]
pub fn derive_persist(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    // Parse the input tokens into a syntax tree.
    let input = parse_macro_input!(input as DeriveInput);

    if !validate_repr(&input.attrs) {
        panic!("wrong repr")
    }

    // Used in the quasi-quotation below as `#name`.
    let name = input.ident;
    let vis = input.vis;

    let errname = syn::Ident::new(&format!("{}ValidateError", name), Span::call_site());

    // Add a bound `T: HeapSize` to every type parameter T.
    let generics = add_trait_bounds(input.generics);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let validate = persist_validate(&input.data, &errname);
    let write_canonical = persist_write_canonical(&input.data);


    let expanded = quote! {
        #[derive(Debug,PartialEq,Eq)]
        #vis struct #errname(());

        // The generated impl.
        #[automatically_derived]
        unsafe impl #impl_generics ::persist::Persist for #name #ty_generics #where_clause {
            type Error = #errname;

            #[inline]
            fn validate(__maybe: &::persist::MaybeValid<Self>) -> Result<&Self, Self::Error> {
                #validate
            }

            #[inline]
            fn write_canonical<'__b>(&self, mut __dst: ::persist::UninitBytes<'__b, Self>) -> &'__b mut [u8] {
                #write_canonical
            }
        }
    };

    // Hand the output tokens back to the compiler.
    proc_macro::TokenStream::from(expanded)
}

// Add a bound `T: HeapSize` to every type parameter T.
fn add_trait_bounds(mut generics: Generics) -> Generics {
    for param in &mut generics.params {
        if let GenericParam::Type(ref mut type_param) = *param {
            type_param.bounds.push(parse_quote!(::persist::Persist));
        }
    }
    generics
}

// Generate an expression to sum up the heap size of each field.
fn persist_validate(data: &Data, errname: &syn::Ident) -> TokenStream {
    match *data {
        Data::Struct(ref data) => {
            match data.fields {
                Fields::Named(ref fields) => {
                    let recurse = fields.named.iter().map(|f| {
                        let ty = &f.ty;
                        quote_spanned! {f.span()=>
                            let __maybe = __maybe.field::<#ty>()
                                                 .map_err(|_| #errname(()))?;
                        }
                    });
                    quote! {
                        let __maybe = __maybe.validate_fields();
                        #(#recurse)*

                        unsafe {
                            Ok(__maybe.assume_valid())
                        }
                    }
                }
                Fields::Unnamed(ref fields) => {
                    let recurse = fields.unnamed.iter().map(|f| {
                        let ty = &f.ty;
                        quote_spanned! {f.span()=>
                            let __maybe = __maybe.field::<#ty>()
                                                 .map_err(|_| #errname(()))?;
                        }
                    });
                    quote! {
                        let __maybe = __maybe.validate_fields();
                        #(#recurse)*

                        unsafe {
                            Ok(__maybe.assume_valid())
                        }
                    }
                }
                Fields::Unit => {
                    quote! {
                        unsafe { Ok(__maybe.assume_valid()) }
                    }
                }
            }
        }
        Data::Enum(_) | Data::Union(_) => unimplemented!(),
    }
}

fn persist_write_canonical(data: &Data) -> TokenStream {
    match *data {
        Data::Struct(ref data) => {
            match data.fields {
                Fields::Named(ref fields) => {
                    let recurse = fields.named.iter().map(|f| {
                        let name = &f.ident;
                        quote_spanned! {f.span()=>
                            __dst.write(&self.#name);
                        }
                    });
                    quote! {
                        #( #recurse )*

                        __dst.done()
                    }
                }
                Fields::Unnamed(ref fields) => {
                    let recurse = fields.unnamed.iter().enumerate().map(|(i, f)| {
                        let index = Index::from(i);
                        quote_spanned! {f.span()=>
                            __dst.write(&self.#index);
                        }
                    });
                    quote! {
                        #( #recurse )*

                        __dst.done()
                    }
                }
                Fields::Unit => {
                    quote! {
                        __dst.done()
                    }
                }
            }
        }
        Data::Enum(_) | Data::Union(_) => unimplemented!(),
    }
}
