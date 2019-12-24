#![feature(never_type)]

use hoard::prelude::*;
use hoard::marshal::prelude::*;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Outpoint {
    txid: [u8;32],
    n: Le<u32>,
}

impl<Z> Encoded<Z> for Outpoint {
    type Encoded = Self;
}

impl<Z: Zone> Encode<'_, Z> for Outpoint {
    type State = ();

    #[inline(always)]
    fn save_children(&self) -> () {}

    #[inline(always)]
    fn poll<D: Dumper<Z>>(&self, _: &mut (), dumper: D) -> Result<D, D::Error> {
        Ok(dumper)
    }

    #[inline(always)]
    fn encode_blob<W: WriteBlob>(&self, _: &(), dst: W) -> Result<W::Ok, W::Error> {
        dst.write_primitive(&self.txid)?
            .write_primitive(&self.n)?
            .finish()
    }
}

impl Persist for Outpoint {
    type Persist = Self;
}

impl ValidateBlob for Outpoint {
    type Error = !;

    #[inline(always)]
    fn validate_blob<B: BlobValidator<Self>>(blob: B) -> Result<B::Ok, B::Error> {
        let mut blob = blob.validate_struct();
        blob.field::<[u8;32],_>(Into::into)?;
        blob.field::<Le<u32>,_>(Into::into)?;
        unsafe { blob.assume_valid() }
    }
}

unsafe impl<'a,Z> ValidateChildren<'a, Z> for Outpoint {
    type State = (<[u8;32] as ValidateChildren<'a, Z>>::State,
                  <Le<u32> as ValidateChildren<'a, Z>>::State);

    #[inline(always)]
    fn validate_children(this: &'a Self) -> Self::State {
        (<[u8;32] as ValidateChildren<'a, Z>>::validate_children(&this.txid),
         <Le<u32> as ValidateChildren<'a, Z>>::validate_children(&this.n))
    }

    #[inline(always)]
    fn poll<V: PtrValidator<Z>>(this: &'a Self, state: &mut Self::State, validator: &V) -> Result<&'a Self, V::Error> {
        <[u8;32] as ValidateChildren<'a, Z>>::poll(&this.txid, &mut state.0, validator)?;
        <Le<u32> as ValidateChildren<'a, Z>>::poll(&this.n, &mut state.1, validator)?;
        Ok(unsafe { ::core::mem::transmute(this) })
    }
}
impl<Z> Decode<Z> for Outpoint {}

/*
#[repr(C)]
#[derive(Debug)]
pub struct TxOut<Z: Zone> {
    value: Le<u64>,
    script: OwnedPtr<[u8], Z>,
}

impl<Y: Zone, Z: Zone> Encoded<Y> for TxOut<Z> {
    type Encoded = TxOut<Y>;
}

impl<'a, Y: Zone, Z: Zone> Encode<'a, Y> for TxOut<Z>
where Z: Encode<'a, Y>
{
    type State = (<Le<u64> as Encode<'a, Y>>::State,
                  <OwnedPtr<[u8], Z> as Encode<'a, Y>>::State);

    fn save_children(&'a self) -> Self::State {
        (Encode::<'a,Y>::save_children(&self.value),
         Encode::<'a,Y>::save_children(&self.script))
    }

    fn poll<D: Dumper<Y>>(&self, state: &mut Self::State, dumper: D) -> Result<D, D::Error> {
        let dumper = Encode::poll(&self.value, &mut state.0, dumper)?;
        let dumper = Encode::poll(&self.script, &mut state.1, dumper)?;
        Ok(dumper)
    }

    fn encode_blob<W: WriteBlob>(&self, state: &Self::State, dst: W) -> Result<W::Ok, W::Error> {
        dst.write::<Y,_>(&self.value, &state.0)?
           .write::<Y,_>(&self.script, &state.1)?
           .finish()
    }
}

impl<Z: Zone> Validate for TxOut<Z> {
    type Error = <OwnedPtr<[u8], Z> as Validate>::Error;

    #[inline(always)]
    fn validate<B: BlobValidator<Self>>(blob: B) -> Result<B::Ok, B::Error> {
        let mut blob = blob.validate_struct();
        blob.field::<Le<u64>,_>(|x| match x {})?;
        blob.field::<OwnedPtr<[u8], Z>,_>(Into::into)?;
        unsafe { blob.assume_valid() }
    }
}

unsafe impl<Z: Zone> Load<Z> for TxOut<Z> {
    type ValidateChildren = (<Le<u64> as Load<Z>>::ValidateChildren, <OwnedPtr<[u8], Z> as Load<Z>>::ValidateChildren);

    #[inline(always)]
    fn validate_children(&self) -> Self::ValidateChildren {
        (Load::<Z>::validate_children(&self.value),
         Load::<Z>::validate_children(&self.script))
    }
}
*/
