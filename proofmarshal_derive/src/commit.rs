use std::convert::TryFrom;

use proc_macro2::{TokenStream, Span};
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{parse_macro_input, parse_quote, Data, DeriveInput, Fields, GenericParam, TypeParam, Generics, Index};
use syn::parse_quote::ParseQuote;

pub fn derive_commit(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    // Parse the input tokens into a syntax tree.
    let input = parse_macro_input!(input as DeriveInput);

    // Used in the quasi-quotation below as `#name`.
    let name = input.ident;
    let vis = input.vis;

    let generics = add_trait_bounds(input.generics);
    let mut generics2 = generics.clone();

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();


    /*
    generics2.params.push(parse_quote!(#ptr_ty));
    let (impl_generics, _, _) = generics2.split_for_impl();
    */

    //let encode = verbatim_encode(&input.data, &ptr_ty);

    let expanded = quote! {
        // The generated impl.
        #[automatically_derived]
        impl #impl_generics ::proofmarshal::commit::Commit for #name #ty_generics #where_clause {
            type Committed = #name #ty_generics;

            #[inline]
            fn commit(&self) -> ::proofmarshal::digest::Digest<Self::Committed>
            {
                let __len: usize = <#name #ty_generics as ::verbatim::Verbatim<()>>::LEN;

		let mut __stack = [0u8;128];
		let mut __heap;

		let mut __buf = if __len > __stack.len() {
		    __heap = vec![0; __len];
		    &mut __heap[..]
		} else {
		    &mut __stack[0..__len]
		};

		<Self as ::verbatim::Verbatim<()>>::encode(self, &mut __buf[..], &mut ())
                                                   .expect("writing to a buffer is infallible");

		::proofmarshal::digest::Digest::hash_verbatim_bytes(
                    __buf,
                    <#name #ty_generics as ::verbatim::Verbatim<()>>::NONZERO_NICHE
                )
            }
        }
    };

    // Hand the output tokens back to the compiler.
    proc_macro::TokenStream::from(expanded)
}

fn add_trait_bounds(mut generics: Generics) -> Generics {
    for param in &mut generics.params {
        if let GenericParam::Type(ref mut type_param) = *param {
            type_param.bounds.push(parse_quote!(::verbatim::Verbatim<()>));
        }
    }
    generics
}
