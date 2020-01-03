use leint::Le;
use proofmarshal_derive::Commit;

//#[derive(Commit)]
//pub struct Foo {}

#[derive(Commit)]
pub struct Outpoint {
    pub txid: [u8;32],
    pub n: Le<u32>,
}

#[derive(Commit)]
pub enum Node {
    Leaf(u8),
    Inner {
        left: u8,
        right: u8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use proofmarshal_core::commit::Verbatim;

    #[test]
    fn test_struct() {
        let outpoint = Outpoint {
            txid: [22;32],
            n: 11.into(),
        };

        assert_eq!(
            outpoint.encode_verbatim(vec![]).unwrap(),
            &[22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 11, 0, 0, 0][..]
        );
    }

    #[test]
    fn test_enum() {
        assert_eq!(Node::LEN, 3);
        assert_eq!(
            Node::Leaf(11).encode_verbatim(vec![]).unwrap(),
            &[0, 11, 0][..]
        );
        assert_eq!(
            Node::Inner { left: 13, right: 14 }.encode_verbatim(vec![]).unwrap(),
            &[1, 13, 14][..]
        );
    }
}
