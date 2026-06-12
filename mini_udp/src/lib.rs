#[cfg(test)]
extern crate self as mini_udp;

mod bit_repr;
pub use bit_repr::{BitRepr, BitReprError};

pub use mini_udp_derive::BitRepr;

#[cfg(test)]
mod test {
    use super::*;

    // #[test]
    // fn it_compiles() {
    //     #[derive(BitRepr, PartialEq, Debug)]
    //     enum X {
    //         Y(u32),
    //         Z,
    //     }
    //     #[derive(BitRepr)]
    //     enum Y {
    //         Z,
    //     }
    //     #[derive(BitRepr)]
    //     enum Z {
    //         A,
    //         B,
    //         C,
    //     }
    //     #[derive(BitRepr)]
    //     enum ZX {
    //         A,
    //         B,
    //         C,
    //         D,
    //     }
    //     #[derive(BitRepr)]
    //     enum ZY {
    //         A,
    //         B,
    //         C,
    //         D,
    //         E,
    //     }
    //     assert_eq!(X::Y(0).bit_len(), 1);
    //     assert_eq!(Y::Z.bit_len(), 0);
    //     assert_eq!(X::from_bytes(&[1; 1], 0).unwrap(), X::Z);
    //     // #[derive(BitRepr)]
    //     // enum Z {}
    //     // Z::from_bytes(&[0; 0], 0).unwrap();
    //
    //     assert_eq!(Z::MIN_BIT_LEN, Z::MAX_BIT_LEN);
    //     assert_eq!(Z::MIN_BIT_LEN, 2);
    //     assert_eq!(ZX::MIN_BIT_LEN, ZX::MAX_BIT_LEN);
    //     assert_eq!(ZX::MIN_BIT_LEN, 2);
    //     assert_eq!(ZY::MIN_BIT_LEN, ZY::MAX_BIT_LEN);
    //     assert_eq!(ZY::MIN_BIT_LEN, 3);
    // }

    #[test]
    fn struct_roundtrip() {
        #[derive(BitRepr, PartialEq, Debug)]
        struct A {}
        #[derive(BitRepr, PartialEq, Debug)]
        struct B;
        #[derive(BitRepr, PartialEq, Debug)]
        struct C();
        #[derive(BitRepr, PartialEq, Debug)]
        struct D {
            byte: i8,
        }
        #[derive(BitRepr, PartialEq, Debug)]
        struct E(u8);
        #[derive(BitRepr, PartialEq, Debug)]
        struct F {
            first: u8,
            second: u8,
            third: u8,
        }
        #[derive(BitRepr, PartialEq, Debug)]
        struct G(u8, u8);

        crate::test_bitrepr_roundtrip!(a, A, A {});
        crate::test_bitrepr_roundtrip!(b, B, B);
        crate::test_bitrepr_roundtrip!(c, C, C());
        crate::test_bitrepr_roundtrip!(d, D, D { byte: 0 });
        crate::test_bitrepr_roundtrip!(d, D, D { byte: -128 });
        crate::test_bitrepr_roundtrip!(d, D, D { byte: -1 });
        crate::test_bitrepr_roundtrip!(d, D, D { byte: 127 });
        crate::test_bitrepr_roundtrip!(e, E, E(249));
        crate::test_bitrepr_roundtrip!(
            f,
            F,
            F {
                first: 255,
                second: 0,
                third: 119
            }
        );
        crate::test_bitrepr_roundtrip!(g, G, G(2, 1));
    }

    #[test]
    fn enum_roundtrip() {
        #[derive(BitRepr, PartialEq, Debug)]
        enum A {
            B,
        }
        #[derive(BitRepr, PartialEq, Debug)]
        enum B {
            A,
            B,
            C,
        }
        #[derive(BitRepr, PartialEq, Debug)]
        enum C {
            B(u8),
            C { byte0: u8, byte1: u8, float: f32 },
            D,
            E(f64, u8, f32, f64),
        }

        crate::test_bitrepr_roundtrip!(a, A, A::B);
        crate::test_bitrepr_roundtrip!(b, B, B::B);
        crate::test_bitrepr_roundtrip!(c, C, C::B(129));
        crate::test_bitrepr_roundtrip!(c, C, C::D);
        crate::test_bitrepr_roundtrip!(
            c,
            C,
            C::C {
                byte0: 0,
                byte1: 255,
                float: 1031.420123
            }
        );
        crate::test_bitrepr_roundtrip!(c, C, C::E(f64::MAX, 4, -193042.04, f64::MIN));
    }

    #[macro_export]
    macro_rules! test_bitrepr_roundtrip {
        ($binding:ident, $ty:ident, $init:expr) => {
            let mut buf = [0; $ty::MAX_BIT_LEN];
            let $binding = $init;
            assert!($binding.write_to_bytes(&mut buf).is_ok());
            assert_eq!($ty::from_bytes(&buf).unwrap(), $binding);
        };
    }
}
