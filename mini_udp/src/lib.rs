#[cfg(test)]
extern crate self as mini_udp;

pub use mini_udp_derive::BitRepr;

mod bit_repr;
pub use bit_repr::{BitRepr, BitReprError, BitReprExt, StaticBitRepr};

mod packet;
mod packet_ack;
mod ring_buffer;

mod sender;
pub use sender::{MultiUdpCommunicator, UdpCommunicator};

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn min_max_length_derive() {
        #[derive(BitRepr)]
        struct AA {}
        assert_eq!(AA::MIN_BIT_LEN, 0);
        assert_eq!(AA::MAX_BIT_LEN, 0);

        #[derive(BitRepr)]
        struct AB();
        assert_eq!(AB::MIN_BIT_LEN, 0);
        assert_eq!(AB::MAX_BIT_LEN, 0);

        #[derive(BitRepr)]
        struct AC;
        assert_eq!(AC::MIN_BIT_LEN, 0);
        assert_eq!(AC::MAX_BIT_LEN, 0);

        #[derive(BitRepr)]
        enum AD {
            A,
        }
        assert_eq!(AD::MIN_BIT_LEN, 0);
        assert_eq!(AD::MAX_BIT_LEN, 0);

        #[derive(BitRepr)]
        struct BA {
            n: u8,
        }
        assert_eq!(BA::MIN_BIT_LEN, 1);
        assert_eq!(BA::MAX_BIT_LEN, 1);

        #[derive(BitRepr)]
        struct BB(u8);
        assert_eq!(BB::MIN_BIT_LEN, 1);
        assert_eq!(BB::MAX_BIT_LEN, 1);

        #[derive(BitRepr)]
        enum BC {
            A(u8),
        }
        assert_eq!(BC::MIN_BIT_LEN, 1);
        assert_eq!(BC::MAX_BIT_LEN, 1);

        #[derive(BitRepr)]
        struct CA {
            named: f32,
            unnamed: bool,
            b: u128,
        }
        assert_eq!(CA::MIN_BIT_LEN, 21);
        assert_eq!(CA::MAX_BIT_LEN, 21);

        #[derive(BitRepr)]
        struct CB(i64, i8);
        assert_eq!(CB::MIN_BIT_LEN, 9);
        assert_eq!(CB::MAX_BIT_LEN, 9);

        #[derive(BitRepr)]
        enum CC {
            A { x: u32 },
            B(bool, u16),
            C(i128),
        }
        assert_eq!(CC::MIN_BIT_LEN, 4);
        assert_eq!(CC::MAX_BIT_LEN, 17);

        #[derive(BitRepr)]
        struct DA(CC);
        assert_eq!(DA::MIN_BIT_LEN, 4);
        assert_eq!(DA::MAX_BIT_LEN, 17);

        #[derive(BitRepr)]
        struct DB {
            a: bool,
            b: i32,
            c: DA,
        }
        assert_eq!(DB::MIN_BIT_LEN, 9);
        assert_eq!(DB::MAX_BIT_LEN, 22);

        #[derive(BitRepr)]
        enum DC {
            A,
            B(u16),
            C { nested: CB },
        }
        assert_eq!(DC::MIN_BIT_LEN, 1);
        assert_eq!(DC::MAX_BIT_LEN, 10);

        #[derive(BitRepr)]
        enum DD {
            B(u64, bool, u8),
            C { nested: CB },
            BC(BC, i8),
        }
        assert_eq!(DD::MIN_BIT_LEN, 3);
        assert_eq!(DD::MAX_BIT_LEN, 11);

        #[derive(BitRepr)]
        enum DE {
            X(bool, BA, u16),
            Y { fixed: DB, x: u8 },
            Z(bool, i8),
        }
        assert_eq!(DE::MIN_BIT_LEN, 3);
        assert_eq!(DE::MAX_BIT_LEN, 24);

        #[derive(BitRepr)]
        enum DF {
            X(bool, BA, u16),
            Y { fixed: DB, x: u8 },
            Z(CA, i16, BA),
        }
        assert_eq!(DF::MIN_BIT_LEN, 5);
        assert_eq!(DF::MAX_BIT_LEN, 25);
    }

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
            first: u16,
            second: isize,
            third: u8,
        }
        #[derive(BitRepr, PartialEq, Debug)]
        struct G(u8, i128);

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
                first: 61008,
                second: -90_502,
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
            C { byte0: u8, byte1: u64, float: f32 },
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

    #[test]
    fn delegated() {
        #[derive(BitRepr, PartialEq, Debug)]
        struct A {
            x: f32,
            y: B,
        }
        #[derive(BitRepr, PartialEq, Debug)]
        struct B(u128);
    }

    #[cfg(feature = "byteable")]
    #[test]
    fn byteable_enum_roundtrip() {
        use byteable::Byteable;
        #[derive(Byteable, PartialEq, Debug, Clone, Copy)]
        enum A {
            B,
        }
        #[derive(Byteable, PartialEq, Debug, Clone, Copy)]
        enum B {
            A,
            B,
            C,
        }
        #[derive(Byteable, PartialEq, Debug)]
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
