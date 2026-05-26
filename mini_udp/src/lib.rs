#[cfg(test)]
extern crate self as mini_udp;

mod bit_repr;
pub use bit_repr::{BitRepr, BitReprError};

pub use mini_udp_derive::BitRepr;

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn it_compiles() {
        #[derive(BitRepr, PartialEq, Debug)]
        enum X {
            Y,
            Z,
        }
        #[derive(BitRepr)]
        enum Y {
            Z,
        }
        #[derive(BitRepr)]
        enum Z {
            A,
            B,
            C,
        }
        #[derive(BitRepr)]
        enum ZX {
            A,
            B,
            C,
            D,
        }
        #[derive(BitRepr)]
        enum ZY {
            A,
            B,
            C,
            D,
            E,
        }
        assert_eq!(X::Y.bit_len(), 1);
        assert_eq!(Y::Z.bit_len(), 0);
        assert_eq!(X::from_bytes(&[1; 1], 0).unwrap(), X::Z);
        // #[derive(BitRepr)]
        // enum Z {}
        // Z::from_bytes(&[0; 0], 0).unwrap();

        assert_eq!(Z::MIN_BIT_LEN, Z::MAX_BIT_LEN);
        assert_eq!(Z::MIN_BIT_LEN, 2);
        assert_eq!(ZX::MIN_BIT_LEN, ZX::MAX_BIT_LEN);
        assert_eq!(ZX::MIN_BIT_LEN, 2);
        assert_eq!(ZY::MIN_BIT_LEN, ZY::MAX_BIT_LEN);
        assert_eq!(ZY::MIN_BIT_LEN, 3);
    }
}
