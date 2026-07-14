extern crate self as mini_udp;

mod byte_repr;
mod packet;
mod packet_ack;
mod prelude;
mod ring_buffer;
mod sender;

use crate::prelude::*;

#[doc(hidden)]
pub use tracing;

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn min_max_length_derive() {
        #[derive(ByteRepr)]
        struct AA {}
        assert_eq!(AA::MIN_BYTE_LEN, 0);
        assert_eq!(AA::MAX_BYTE_LEN, 0);

        #[derive(ByteRepr)]
        struct AB();
        assert_eq!(AB::MIN_BYTE_LEN, 0);
        assert_eq!(AB::MAX_BYTE_LEN, 0);

        #[derive(ByteRepr)]
        struct AC;
        assert_eq!(AC::MIN_BYTE_LEN, 0);
        assert_eq!(AC::MAX_BYTE_LEN, 0);

        #[derive(ByteRepr)]
        enum AD {
            A,
        }
        assert_eq!(AD::MIN_BYTE_LEN, 0);
        assert_eq!(AD::MAX_BYTE_LEN, 0);

        #[derive(ByteRepr)]
        struct BA {
            n: u8,
        }
        assert_eq!(BA::MIN_BYTE_LEN, 1);
        assert_eq!(BA::MAX_BYTE_LEN, 1);

        #[derive(ByteRepr)]
        struct BB(u8);
        assert_eq!(BB::MIN_BYTE_LEN, 1);
        assert_eq!(BB::MAX_BYTE_LEN, 1);

        #[derive(ByteRepr)]
        enum BC {
            A(u8),
        }
        assert_eq!(BC::MIN_BYTE_LEN, 1);
        assert_eq!(BC::MAX_BYTE_LEN, 1);

        #[derive(ByteRepr)]
        struct CA {
            named: f32,
            unnamed: bool,
            b: u128,
        }
        assert_eq!(CA::MIN_BYTE_LEN, 21);
        assert_eq!(CA::MAX_BYTE_LEN, 21);

        #[derive(ByteRepr)]
        struct CB(i64, i8);
        assert_eq!(CB::MIN_BYTE_LEN, 9);
        assert_eq!(CB::MAX_BYTE_LEN, 9);

        #[derive(ByteRepr)]
        enum CC {
            A { x: u32 },
            B(bool, u16),
            C(i128),
        }
        assert_eq!(CC::MIN_BYTE_LEN, 4);
        assert_eq!(CC::MAX_BYTE_LEN, 17);

        #[derive(ByteRepr)]
        struct DA(CC);
        assert_eq!(DA::MIN_BYTE_LEN, 4);
        assert_eq!(DA::MAX_BYTE_LEN, 17);

        #[derive(ByteRepr)]
        struct DB {
            a: bool,
            b: i32,
            c: DA,
        }
        assert_eq!(DB::MIN_BYTE_LEN, 9);
        assert_eq!(DB::MAX_BYTE_LEN, 22);

        #[derive(ByteRepr)]
        enum DC {
            A,
            B(u16),
            C { nested: CB },
        }
        assert_eq!(DC::MIN_BYTE_LEN, 1);
        assert_eq!(DC::MAX_BYTE_LEN, 10);

        #[derive(ByteRepr)]
        enum DD {
            B(u64, bool, u8),
            C { nested: CB },
            BC(BC, i8),
        }
        assert_eq!(DD::MIN_BYTE_LEN, 3);
        assert_eq!(DD::MAX_BYTE_LEN, 11);

        #[derive(ByteRepr)]
        enum DE {
            X(bool, BA, u16),
            Y { fixed: DB, x: u8 },
            Z(bool, i8),
        }
        assert_eq!(DE::MIN_BYTE_LEN, 3);
        assert_eq!(DE::MAX_BYTE_LEN, 24);

        #[derive(ByteRepr)]
        enum DF {
            X(bool, BA, u16),
            Y { fixed: DB, x: u8 },
            Z(CA, i16, BA),
        }
        assert_eq!(DF::MIN_BYTE_LEN, 5);
        assert_eq!(DF::MAX_BYTE_LEN, 25);
    }

    #[test]
    fn struct_roundtrip() {
        #[derive(ByteRepr, PartialEq, Debug)]
        struct A {}
        #[derive(ByteRepr, PartialEq, Debug)]
        struct B;
        #[derive(ByteRepr, PartialEq, Debug)]
        struct C();
        #[derive(ByteRepr, PartialEq, Debug)]
        struct D {
            byte: i8,
        }
        #[derive(ByteRepr, PartialEq, Debug)]
        struct E(u8);
        #[derive(ByteRepr, PartialEq, Debug)]
        struct F {
            first: u16,
            second: isize,
            third: u8,
        }
        #[derive(ByteRepr, PartialEq, Debug)]
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
        #[derive(ByteRepr, PartialEq, Debug)]
        enum A {
            B,
        }
        #[derive(ByteRepr, PartialEq, Debug)]
        enum B {
            A,
            B,
            C,
        }
        #[derive(ByteRepr, PartialEq, Debug)]
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
        #[derive(ByteRepr, PartialEq, Debug)]
        struct A {
            x: f32,
            y: B,
        }
        #[derive(ByteRepr, PartialEq, Debug)]
        struct B(u128);

        crate::test_bitrepr_roundtrip!(
            a,
            A,
            A {
                x: -2409406.3353,
                y: B(3908221)
            }
        );
    }

    #[test]
    fn vector() {
        #[derive(ByteRepr, PartialEq, Debug)]
        struct A {
            list: Vec<u8>,
        }
        #[derive(ByteRepr, PartialEq, Debug)]
        struct B(f64);
        #[derive(ByteRepr, PartialEq, Debug)]
        struct C {
            a: Vec<bool>,
            b: Vec<B>,
        }

        crate::test_bitrepr_roundtrip!(
            a,
            A,
            A {
                list: vec![183, 1, 99, 254]
            }
        );
        crate::test_bitrepr_roundtrip!(
            c,
            C,
            C {
                a: vec![true, false, false, false, true, false],
                b: vec![B(f32::MAX as f64), B(f64::MIN), B(-0.005), B(01208402.2432)]
            }
        );
    }

    #[macro_export]
    macro_rules! test_bitrepr_roundtrip {
        ($binding:ident, $ty:ident, $init:expr) => {
            let mut buf = [0; $ty::MAX_BYTE_LEN];
            let $binding = $init;
            assert!($binding.write_to_bytes(&mut buf).is_ok());
            assert_eq!($ty::from_bytes(&buf).unwrap(), $binding);
        };
    }
}
