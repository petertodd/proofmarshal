use super::Type;

/// fixme
#[macro_export]
macro_rules! impl_verbatim_for_primitive_type {
    (
        $(#[$attr:meta])*
        $vis:vis struct $name:ident $( < $( $generic:ident ),* > )? {
            $( $field_vis:vis $field_id:ident : $field_ty:ty ),* $(,)?
        }
    ) => {
        #[automatically_derived]
        impl<__P: $crate::verbatim::VerbatimPtr, $( $($generic),* )?>
            $crate::verbatim::Verbatim<__P> for $name $( < $($generic),* > )?
        $( where $( $generic: $crate::verbatim::Verbatim<__P> + $crate::Type<Metadata=()> ),* )?
        {
            #[inline(always)]
            fn is_nonzero(__metadata: Self::Metadata) -> bool {
                true
                $( && <$field_ty as $crate::verbatim::Verbatim<__P>>::is_nonzero(()) )*
            }

            #[inline(always)]
            fn len(__metadata: Self::Metadata) -> usize {
                0
                $( + <$field_ty as $crate::verbatim::Verbatim<__P>>::len(()) )*
            }

            #[inline(always)]
            fn encode<__E, __W>(&self, __metadata: Self::Metadata, __ptr_encoder: &mut __E, __dst: __W)
                -> Result<__W::Done, __E::Error>
                where __E: $crate::verbatim::PtrEncoder<__P>,
                      __W: $crate::verbatim::WriteBytes,
            {
                $(
                    let __dst = self.$field_id.encode((), __ptr_encoder,
                                                      __dst.reserve(<$field_ty as $crate::verbatim::Verbatim<__P>>::len(())))?;
                )*

                Ok(__dst.done())
            }
        }
    };
}

pub struct Foo<T> {
    bar: T,
    car: Option<T>,
    car2: Option<T>,
}

impl<T> Type for Foo<T> {
    type Metadata = ();
}

impl_verbatim_for_primitive_type! {
    pub struct Foo<T> {
        bar: T,
        car: Option<T>,
        car2: Option<T>,
    }
}

/*
macro_rules! __def_primitive {
    (
        $(#[$attr:meta])+
        $vis:vis struct $name:ident < $( $generic:ident ),* > {
            $( $field_vis:vis $field_id:ident : $field_ty:ty ),* $(,)?
        }
    ) => {
        $(#[$attr])+
        $vis struct $name <$($generic),*> {
            $( $field_vis $field_id : $field_ty ),*
        }

        #[automatically_derived]
        unsafe impl<$($generic),*> $crate::arena::types::Type for $name<$($generic),*> {
            fn layout_in<__B: $crate::arena::Arena>() -> ::core::alloc::Layout {
                // Primitive type, so no change to layout.
                ::core::alloc::Layout::new::<Self>()
            }
        }

        #[automatically_derived]
        unsafe impl<$($generic),*> $crate::arena::types::ValueType for $name<$($generic),*> {
            type Type = Self;
        }

        #[automatically_derived]
        unsafe impl<__B: $crate::arena::Arena, $($generic),*> $crate::arena::types::Coerce<__B> for $name<$($generic),*>
            where $( $generic : $crate::arena::types::Coerce<__B, Coerced=$generic> ),*
        {
            type Coerced = Self;
        }

        #[automatically_derived]
        unsafe impl<__A: $crate::arena::Arena, $($generic),*> $crate::arena::types::Value<__A> for $name<$($generic),*>
            where
        {
            fn move_to_unchecked<'__b, __B: '__b + $crate::arena::Arena,
                                         __D: $crate::util::emplace::Emplace<'__b>>
                    (self, src_arena: &__A, dst_arena: &mut __B, dst: __D) -> __D::Done
                //where __B: $crate::arena::types::MoveValueFrom<__A>
            {
                // Primitive type, so we're unchanged. We also can't own any lifetimes from either
                // arena, so unchecked is safe.
                unsafe {
                    dst.emplace_unchecked(self)
                }
            }
        }
    };
    (
        $(#[$attr:meta])+
        $vis:vis struct $name:ident < $( $generic:ident ),* > (
            $( $field_vis:vis $field_ty:ty ),*
        )
    ) => {
        $(#[$attr])+
        $vis struct $name <$($generic),*> (
            $( $field_vis $field_ty ),*
        );

        #[automatically_derived]
        unsafe impl<$($generic),*> $crate::arena::types::Type for $name<$($generic),*> {
            fn layout_in<__B: $crate::arena::Arena>() -> ::core::alloc::Layout {
                // Primitive type, so no change to layout.
                ::core::alloc::Layout::new::<Self>()
            }
        }

        #[automatically_derived]
        unsafe impl<$($generic),*> $crate::arena::types::ValueType for $name<$($generic),*> {
            type Type = Self;
        }

        #[automatically_derived]
        unsafe impl<__B: $crate::arena::Arena, $($generic),*> $crate::arena::types::Coerce<__B> for $name<$($generic),*>
            where $( $generic : $crate::arena::types::Coerce<__B, Coerced=$generic> ),*
        {
            type Coerced = Self;
        }

        #[automatically_derived]
        unsafe impl<__A: $crate::arena::Arena, $($generic),*> $crate::arena::types::Value<__A> for $name<$($generic),*>
            where
        {
            fn move_to_unchecked<'__b, __B: '__b + $crate::arena::Arena,
                                         __D: $crate::util::emplace::Emplace<'__b>>
                    (self, src_arena: &__A, dst_arena: &mut __B, dst: __D) -> __D::Done
                //where __B: $crate::arena::types::MoveValueFrom<__A>
            {
                // Primitive type, so we're unchanged. We also can't own any lifetimes from either
                // arena, so unchecked is safe.
                unsafe {
                    dst.emplace_unchecked(self)
                }
            }
        }
    };
}

def_primitive! {
    #[repr(C)]
    struct Foo<T> {
        bar: T,
        car: Option<T>,
    }
}

def_primitive! {
    #[repr(transparent)]
    struct Bar {
        bar: (),
        car: Option<u8>,
    }
}

def_primitive! {
    #[repr(C)]
    struct Baz (
        bool,
        Option<u8>,
    );
}



macro_rules! def_type {
    (
        #[repr(C)]
        $(#[$attr:meta])*
        $vis:vis struct $name:ident <$($generic:ident),* @ $arena:ident : Arena> {
            $( $field_vis:vis $field_id:ident : $field_ty:ty ),* $(,)?
        }
    ) => {
        __def_type! {
            #[repr(C)]
            $(#[$attr])*
            $vis struct $name <$( $generic),* @ $arena> {
                $( $field_vis $field_id : $field_ty ),*
            }
        }
    };
}

macro_rules! __def_type {
    (
        $(#[$attr:meta])+
        $vis:vis struct $name:ident <$($generic:ident),* @ $arena:ident> {
            $( $field_vis:vis $field_id:ident : $field_ty:ty ),+
        }
    ) => {
        $(#[$attr])+
        $vis struct $name <$($generic,)* $arena : $crate::arena::Arena = !> {
            $( $field_vis $field_id : $field_ty ),*
        }

        unsafe impl<$($generic: $crate::arena::types::Type,)* $arena: $crate::arena::Arena>
            $crate::arena::types::Type for $name<$($generic,)* $arena>
        {
            fn layout_in<__B: $crate::arena::Arena>() -> ::core::alloc::Layout {
                unimplemented!()
            }
        }

        unsafe impl<$($generic : $crate::arena::types::ValueType,)* $arena: $crate::arena::Arena>
            $crate::arena::types::ValueType for $name<$($generic,)* $arena>
        {
            type Type = $name<$( <$generic as $crate::arena::types::ValueType>::Type,)* !>;
        }

        unsafe impl<__B: $crate::arena::Arena,
                    $($generic: $crate::arena::types::Coerce<__B>,)*
                    $arena: $crate::arena::Arena>
            $crate::arena::types::Coerce<__B> for $name<$($generic,)* $arena>
            where $( $generic : $crate::arena::types::Coerce<__B, Coerced=$generic> ),*
        {
            type Coerced = $name<$( <$generic as $crate::arena::types::Coerce<__B>>::Coerced, )* __B>;
        }

        unsafe impl<$($generic: $crate::arena::types::Value<$arena>,)*
                    $arena: $crate::arena::Arena>
            $crate::arena::types::Value<$arena> for $name<$($generic,)* $arena>
            where
        {
            fn move_to_unchecked<'__b, __B: '__b + $crate::arena::Arena,
                                         __D: $crate::util::emplace::Emplace<'__b>>
                    (self, src_arena: &$arena, dst_arena: &mut __B, dst: __D) -> __D::Done
                where __B: $crate::arena::types::MoveValueFrom<$arena>
            {
                /*
                // Define a generic structure with the same layout as ourselves
                #[repr(C)]
                struct __Generic<$( $generic,)* $arena: $crate::arena::Arena>($( $field_ty ),*);

                // Now use that generic structure to reconstruct ourselves!
                let __r = __Generic(
                    $( {
                        // FIXME: need to do handle different sized things...
                        let __r: $crate::util::emplace::MaybeValid<::core::mem::MaybeUninit<$field_ty>>
                            = <$field_ty as $crate::arena::types::Value<$arena>>::move_to_unchecked(
                                self.$field_id, src_arena, dst_arena, ::core::marker::PhantomData
                            );
                        unsafe { __r.assume_valid() }
                    }),*
                );

                unsafe {
                    dst.emplace_unchecked(__r)
                }
                */
                unimplemented!()
            }
        }
    };
}

use crate::arena::Own;
def_type! {
    #[repr(C)]
    struct FooA<T @ A: Arena> {
        a: Own<T,A>,
        b: u8,
        c: bool,
    }
}*/
