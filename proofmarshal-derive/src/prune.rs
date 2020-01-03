use super::*;

pub fn derive_prune(mut s: synstructure::Structure) -> proc_macro2::TokenStream {
    s.bind_with(|_| synstructure::BindStyle::RefMut);

    let prune_body = s.each(|bi| {
        quote! { ::proofmarshal_core::fact::Prune::prune(#bi); }
    });

    let fully_prune_body = s.each(|bi| {
        quote! { ::proofmarshal_core::fact::Prune::fully_prune(#bi); }
    });

    let r = s.gen_impl(quote! {
        gen impl ::proofmarshal_core::fact::Prune for @Self {
            fn prune(&mut self) {
                match self {
                    #prune_body
                }
            }

            fn fully_prune(&mut self) {
                match self {
                    #fully_prune_body
                }
            }
        }
    });
    // eprintln!("{}", synstructure::unpretty_print(&r));
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        synstructure::test_derive!{
            derive_prune {
                struct Foo;
            }
            expands to {
                #[allow(non_upper_case_globals)]
                const _DERIVE_proofmarshal_core_fact_Prune_FOR_Foo: () = {
                    impl ::proofmarshal_core::fact::Prune for Foo {
                        fn prune(&mut self) {
                            match self {
                                Foo => {}
                            }
                        }

                        fn fully_prune(&mut self) {
                            match self {
                                Foo => {}
                            }
                        }
                    }
                };
            } // no_build
        }

        synstructure::test_derive!{
            derive_prune {
                struct Foo(u8);
            }
            expands to {
# [
    allow (
        non_upper_case_globals )
    ]
const _DERIVE_proofmarshal_core_fact_Prune_FOR_Foo : (
    )
= {
    impl :: proofmarshal_core :: fact :: Prune for Foo {
        fn prune (
            & mut self )
        {
            match self {
                Foo (
                    ref mut __binding_0 , )
                => {
                    {
                        :: proofmarshal_core :: fact :: Prune :: prune (
                            __binding_0 )
                        ;
                        }
                    }
                }
            }
        fn fully_prune (
            & mut self )
        {
            match self {
                Foo (
                    ref mut __binding_0 , )
                => {
                    {
                        :: proofmarshal_core :: fact :: Prune :: fully_prune (
                            __binding_0 )
                        ;
                        }
                    }
                }
            }
        }
    }
;
            } // no_build
        }

        synstructure::test_derive!{
            derive_prune {
                enum Foo {
                    Bar,
                    Baz(u8),
                }
            }
            expands to {

# [
    allow (
        non_upper_case_globals )
    ]
const _DERIVE_proofmarshal_core_fact_Prune_FOR_Foo : (
    )
= {
    impl :: proofmarshal_core :: fact :: Prune for Foo {
        fn prune (
            & mut self )
        {
            match self {
                Foo :: Bar => {
                    }
                Foo :: Baz (
                    ref mut __binding_0 , )
                => {
                    {
                        :: proofmarshal_core :: fact :: Prune :: prune (
                            __binding_0 )
                        ;
                        }
                    }
                }
            }
        fn fully_prune (
            & mut self )
        {
            match self {
                Foo :: Bar => {
                    }
                Foo :: Baz (
                    ref mut __binding_0 , )
                => {
                    {
                        :: proofmarshal_core :: fact :: Prune :: fully_prune (
                            __binding_0 )
                        ;
                        }
                    }
                }
            }
        }
    }
;

            } // no_build
        }
    }
}
