use quote::quote;
use syn;
use synstructure::decl_derive;

decl_derive!([Primitive, attributes(foo)] => derive_primitive);

fn derive_primitive(s: synstructure::Structure) -> proc_macro2::TokenStream {
    let mut fields_ty = vec![];
    match &s.ast().data {
        syn::Data::Struct(data) => {
            match &data.fields {
                syn::Fields::Named(fields) => {
                    fields_ty.extend(fields.named.iter());
                },
                syn::Fields::Unnamed(fields) => {
                    fields_ty.extend(fields.unnamed.iter());
                },
                syn::Fields::Unit => {},
            }
        },
        syn::Data::Enum(_) => {
            todo!()
        },
        syn::Data::Union(_) => {
            panic!("unions not supported")
        },
    }

    let fields_ty = fields_ty.iter().map(|field| &field.ty).map(|ty| quote! { #ty });
    let validate_body = quote ! {
        #( __blob.field::<#fields_ty,_>(|err| todo!())?; )*

        unsafe { __blob.assume_valid() }
    };

    let encode_blob_impl = s.each(|bi| quote! {
        __dst = __dst.write_primitive(#bi)?;
    });

    let t = s.gen_impl(quote! {
        extern crate hoard;

        use hoard::marshal::blob::*;

        #[derive(Debug)]
        pub struct Error(Box<dyn std::error::Error + 'static + Send + Sync>);

        impl ::core::fmt::Display for Error {
            fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                todo!()
            }
        }

        impl ::std::error::Error for Error {
        }

        gen impl ::hoard::marshal::blob::ValidateBlob for @Self {
            type Error = Error;

            fn validate<'__a, __V: ::hoard::marshal::blob::PaddingValidator>(
                mut __blob: BlobCursor<'__a, Self, __V>
            ) -> Result<::hoard::marshal::blob::ValidBlob<'__a, Self>,
                        ::hoard::marshal::blob::BlobError<Self::Error, __V::Error>>
            {
                #validate_body
            }
        }

        gen unsafe impl<'__a, __Z> ::hoard::marshal::decode::ValidateChildren<'__a, __Z> for @Self {
            type State = ();
            fn validate_children(_: &Self) -> Self::State {}

            fn poll<__V>(_: &Self::Persist, _: &mut (), _: &__V) -> Result<(), __V::Error>
                where __V: ::hoard::marshal::PtrValidator<__Z>,
            {
                Ok(())
            }
        }

        gen impl<__Z> ::hoard::marshal::decode::Decode<__Z> for @Self {}

        gen unsafe impl ::hoard::marshal::decode::Persist for @Self {
            type Persist = Self;
            type Error = <Self as ::hoard::marshal::blob::ValidateBlob>::Error;
        }

        gen impl<__Z> ::hoard::marshal::encode::Encoded<__Z> for @Self {
            type Encoded = Self;
        }

        gen impl<__Z> ::hoard::marshal::encode::Encode<'_, __Z> for @Self {
            type State = ();

            fn make_encode_state(&self) {}

            fn encode_poll<__D>(&self, _: &mut (), __dumper: __D) -> Result<__D, __D::Error>
                where __D: ::hoard::marshal::Dumper<__Z>,
            {
                Ok(__dumper)
            }

            fn encode_blob<__W>(&self, _: &(), mut __dst: __W) -> Result<__W::Ok, __W::Error>
                where __W: ::hoard::marshal::blob::WriteBlob,
            {
                match self {
                    #encode_blob_impl
                };

                __dst.finish()
            }
        }
    });

    // eprintln!("{}", t.to_string());

    t
}

