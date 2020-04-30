pub struct Tx<'x, Z> {
    vin: SumMMR<TxIn<'x, Z>, TxInSum, Z>,
    vout: SumMMR<TxOut<Z>, TxOutSum, Z>,
}

pub struct TxIn<'x, Z> {
    outpoint: Outpoint<'x>,
    sig: Own<[u8], Z>,
}

pub struct TxOut<Z> {
    value: u64,
    pubkey: Own<[u8], Z>,
}
