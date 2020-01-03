use std::convert::TryFrom;

use super::*;

pub fn derive_commit(s: synstructure::Structure) -> proc_macro2::TokenStream {
    if s.variants().len() == 1 {
        derive_commit_for_struct(s)
    } else {
        derive_commit_for_enum(s)
    }
}

pub fn derive_commit_for_struct(s: synstructure::Structure) -> proc_macro2::TokenStream {
    let mut field_lens = vec![];
    let body = s.each(|bi| {
        let ty = &bi.ast().ty;
        field_lens.push(quote! { <#ty as ::proofmarshal_core::commit::Verbatim>::LEN });

        quote! {
            __dst = __dst.write(#bi)?;
        }
    });

    let r = s.gen_impl(quote! {
        gen impl ::proofmarshal_core::commit::Verbatim for @Self {
            const LEN: usize = 0 #( + #field_lens )*;

            fn encode_verbatim<__W>(&self, mut __dst: __W) -> Result<__W, __W::Error>
                where __W: ::proofmarshal_core::commit::WriteVerbatim
            {
                match self {
                    #body
                };

                __dst.finish()
            }
        }
    });
    // println!("{}", synstructure::unpretty_print(&r));
    r
}

pub fn derive_commit_for_enum(s: synstructure::Structure) -> proc_macro2::TokenStream {
    let mut variant_lens = vec![];
    let variants = s.variants().iter().enumerate().map(|(idx, vi)| {
        let pat = vi.pat();
        let fields = vi.bindings().iter().map(|bi| {
            quote! {
                __dst = __dst.write(#bi)?;
            }
        });

        let field_lens = vi.bindings().iter().map(|bi| {
            let ty = &bi.ast().ty;
            quote! { <#ty as ::proofmarshal_core::commit::Verbatim>::LEN }
        });

        let variant_len = quote! {
            0 #( + #field_lens )*
        };
        variant_lens.push(variant_len.clone());

        let idx = u8::try_from(idx).expect("enums with > 255 variants not supported");
        quote! {
            #pat => {
                __dst = __dst.write_bytes(&[#idx])?;

                #( #fields )*

                __dst = __dst.write_padding(Self::LEN - 1 - (#variant_len))?;
                __dst.finish()
            },
        }
    }).collect::<Vec<_>>();

    let r = s.gen_impl(quote! {
        gen impl ::proofmarshal_core::commit::Verbatim for @Self {
            const LEN: usize = 1 + {
                let r = 0;

                const fn __max(a: usize, b: usize) -> usize {
                    [b, a][(a > b) as usize]
                };

                #( let r = __max(r, #variant_lens); )*

                r
            };

            fn encode_verbatim<__W>(&self, mut __dst: __W) -> Result<__W, __W::Error>
                where __W: ::proofmarshal_core::commit::WriteVerbatim
            {
                match self {
                    #( #variants )*
                }
            }
        }
    });
    //println!("{}", synstructure::unpretty_print(&r));
    r
}

#[cfg(test)]
mod tests {
    #[test]
    fn test() {
        /*
        synstructure::test_derive!{
            derive_commit {
                struct A;
            }
            expands to {
                #[allow(non_upper_case_globals)]
                const _DERIVE_proofmarshal_core_commit_Verbatim_FOR_A: () = {
                    impl ::proofmarshal_core::commit::Verbatim for A {
                        const LEN: usize = 0;
                        fn encode_verbatim<__W>(&self, mut __dst: __W) -> Result<__W, __W::Error>
                            where __W: ::proofmarshal_core::commit::WriteVerbatim
                        {
                            match self => A => {}
                            __dst.finish()
                        }
                    }
                };
            } no_build
        }

        synstructure::test_derive!{
            derive_commit {
                struct A(u8);
            }
            expands to {
                #[allow(non_upper_case_globals)]
                const _DERIVE_proofmarshal_core_commit_Verbatim_FOR_A: () = {
                    impl ::proofmarshal_core::commit::Verbatim for A {
                        const LEN: usize = 0;
                        fn encode_verbatim<__W>(&self, __dst: __W) -> Result<__W, __W::Error>
                            where __W: ::proofmarshal_core::commit::WriteVerbatim
                        {
                            __dst
                        }
                    }
                };
            } no_build
        }
        */
    }
}
