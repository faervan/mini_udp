extern crate self as mini_udp;

mod byte_repr;
pub use byte_repr::{ByteRepr, ByteReprError};

mod packet;
mod packet_ack;
pub mod prelude;
mod ring_buffer;
mod sender;

#[doc(hidden)]
pub use tracing;

#[cfg(test)]
mod test {
    use crate::prelude::*;

    #[test]
    fn min_max_length_derive() {
        #[derive(ByteRepr)]
        struct AA {}
        assert_eq!(AA::MIN_BYTE_LEN, 0);
        assert_eq!(AA::MAX_BYTE_LEN, 0);
        assert_eq!(AA {}.byte_len(), 0);

        #[derive(ByteRepr)]
        struct AB();
        assert_eq!(AB::MIN_BYTE_LEN, 0);
        assert_eq!(AB::MAX_BYTE_LEN, 0);
        assert_eq!(AB().byte_len(), 0);

        #[derive(ByteRepr)]
        struct AC;
        assert_eq!(AC::MIN_BYTE_LEN, 0);
        assert_eq!(AC::MAX_BYTE_LEN, 0);
        assert_eq!(AC.byte_len(), 0);

        #[derive(ByteRepr)]
        enum AD {
            A,
        }
        assert_eq!(AD::MIN_BYTE_LEN, 0);
        assert_eq!(AD::MAX_BYTE_LEN, 0);
        assert_eq!(AD::A.byte_len(), 0);

        #[derive(ByteRepr)]
        struct BA {
            n: u8,
        }
        assert_eq!(BA::MIN_BYTE_LEN, 1);
        assert_eq!(BA::MAX_BYTE_LEN, 1);
        assert_eq!(BA { n: 0 }.byte_len(), 1);
        assert_eq!(BA { n: 1 }.byte_len(), 1);
        assert_eq!(BA { n: 42 }.byte_len(), 1);
        assert_eq!(BA { n: 127 }.byte_len(), 1);
        assert_eq!(BA { n: 255 }.byte_len(), 1);

        #[derive(ByteRepr)]
        struct BB(u8);
        assert_eq!(BB::MIN_BYTE_LEN, 1);
        assert_eq!(BB::MAX_BYTE_LEN, 1);
        assert_eq!(BB(0).byte_len(), 1);
        assert_eq!(BB(1).byte_len(), 1);
        assert_eq!(BB(42).byte_len(), 1);
        assert_eq!(BB(128).byte_len(), 1);
        assert_eq!(BB(255).byte_len(), 1);

        #[derive(ByteRepr)]
        enum BC {
            A(u8),
        }
        assert_eq!(BC::MIN_BYTE_LEN, 1);
        assert_eq!(BC::MAX_BYTE_LEN, 1);
        assert_eq!(BC::A(0).byte_len(), 1);
        assert_eq!(BC::A(1).byte_len(), 1);
        assert_eq!(BC::A(17).byte_len(), 1);
        assert_eq!(BC::A(99).byte_len(), 1);
        assert_eq!(BC::A(255).byte_len(), 1);

        #[derive(ByteRepr)]
        struct CA {
            named: f32,
            unnamed: bool,
            b: u128,
        }
        assert_eq!(CA::MIN_BYTE_LEN, 21);
        assert_eq!(CA::MAX_BYTE_LEN, 21);
        assert_eq!(
            CA {
                named: 0.0,
                unnamed: false,
                b: 0
            }
            .byte_len(),
            21
        );
        assert_eq!(
            CA {
                named: 1.0,
                unnamed: true,
                b: 1
            }
            .byte_len(),
            21
        );
        assert_eq!(
            CA {
                named: -3.5,
                unnamed: false,
                b: 42
            }
            .byte_len(),
            21
        );
        assert_eq!(
            CA {
                named: 7.25,
                unnamed: true,
                b: 123456
            }
            .byte_len(),
            21
        );
        assert_eq!(
            CA {
                named: f32::INFINITY,
                unnamed: false,
                b: u128::MAX
            }
            .byte_len(),
            21
        );

        #[derive(ByteRepr)]
        struct CB(i64, i8);
        assert_eq!(CB::MIN_BYTE_LEN, 9);
        assert_eq!(CB::MAX_BYTE_LEN, 9);
        assert_eq!(CB(0, 0).byte_len(), 9);
        assert_eq!(CB(1, 1).byte_len(), 9);
        assert_eq!(CB(-1, -1).byte_len(), 9);
        assert_eq!(CB(i64::MIN, i8::MIN).byte_len(), 9);
        assert_eq!(CB(i64::MAX, i8::MAX).byte_len(), 9);

        #[derive(ByteRepr)]
        enum CC {
            A { x: u32 },
            B(bool, u16),
            C(i128),
        }
        assert_eq!(CC::MIN_BYTE_LEN, 4);
        assert_eq!(CC::MAX_BYTE_LEN, 17);
        assert_eq!(CC::A { x: 0 }.byte_len(), 5);
        assert_eq!(CC::A { x: u32::MAX }.byte_len(), 5);
        assert_eq!(CC::B(false, 0).byte_len(), 4);
        assert_eq!(CC::B(true, u16::MAX).byte_len(), 4);
        assert_eq!(CC::C(i128::MIN).byte_len(), 17);

        #[derive(ByteRepr)]
        struct DA(CC);
        assert_eq!(DA::MIN_BYTE_LEN, 4);
        assert_eq!(DA::MAX_BYTE_LEN, 17);
        assert_eq!(DA(CC::A { x: 7 }).byte_len(), 5);
        assert_eq!(DA(CC::B(false, 99)).byte_len(), 4);
        assert_eq!(DA(CC::B(true, 1234)).byte_len(), 4);
        assert_eq!(DA(CC::C(0)).byte_len(), 17);
        assert_eq!(DA(CC::C(i128::MAX)).byte_len(), 17);

        #[derive(ByteRepr)]
        struct DB {
            a: bool,
            b: i32,
            c: DA,
        }
        assert_eq!(DB::MIN_BYTE_LEN, 9);
        assert_eq!(DB::MAX_BYTE_LEN, 22);
        assert_eq!(
            DB {
                a: false,
                b: 0,
                c: DA(CC::A { x: 0 })
            }
            .byte_len(),
            10
        );
        assert_eq!(
            DB {
                a: true,
                b: 1,
                c: DA(CC::A { x: 1 })
            }
            .byte_len(),
            10
        );
        assert_eq!(
            DB {
                a: false,
                b: -1,
                c: DA(CC::B(true, 42))
            }
            .byte_len(),
            9
        );
        assert_eq!(
            DB {
                a: true,
                b: i32::MIN,
                c: DA(CC::C(5))
            }
            .byte_len(),
            22
        );
        assert_eq!(
            DB {
                a: false,
                b: i32::MAX,
                c: DA(CC::C(i128::MAX))
            }
            .byte_len(),
            22
        );

        #[derive(ByteRepr)]
        enum DC {
            A,
            B(u16),
            C { nested: CB },
        }
        assert_eq!(DC::MIN_BYTE_LEN, 1);
        assert_eq!(DC::MAX_BYTE_LEN, 10);
        assert_eq!(DC::A.byte_len(), 1);
        assert_eq!(DC::B(0).byte_len(), 3);
        assert_eq!(DC::B(u16::MAX).byte_len(), 3);
        assert_eq!(DC::C { nested: CB(0, 0) }.byte_len(), 10);
        assert_eq!(
            DC::C {
                nested: CB(i64::MIN, i8::MAX)
            }
            .byte_len(),
            10
        );

        #[derive(ByteRepr)]
        enum DD {
            B(u64, bool, u8),
            C { nested: CB },
            BC(BC, i8),
        }
        assert_eq!(DD::MIN_BYTE_LEN, 3);
        assert_eq!(DD::MAX_BYTE_LEN, 11);
        assert_eq!(DD::B(0, false, 0).byte_len(), 11);
        assert_eq!(DD::B(u64::MAX, true, u8::MAX).byte_len(), 11);
        assert_eq!(
            DD::C {
                nested: CB(123, -5)
            }
            .byte_len(),
            10
        );
        assert_eq!(DD::BC(BC::A(0), 0).byte_len(), 3);
        assert_eq!(DD::BC(BC::A(255), i8::MIN).byte_len(), 3);

        #[derive(ByteRepr)]
        enum DE {
            X(bool, BA, u16),
            Y { fixed: DB, x: u8 },
            Z(bool, i8),
        }
        assert_eq!(DE::MIN_BYTE_LEN, 3);
        assert_eq!(DE::MAX_BYTE_LEN, 24);
        assert_eq!(DE::X(false, BA { n: 0 }, 0).byte_len(), 5);
        assert_eq!(DE::X(true, BA { n: 255 }, u16::MAX).byte_len(), 5);
        assert_eq!(
            DE::Y {
                fixed: DB {
                    a: false,
                    b: 0,
                    c: DA(CC::A { x: 0 }),
                },
                x: 0,
            }
            .byte_len(),
            12
        );
        assert_eq!(
            DE::Y {
                fixed: DB {
                    a: true,
                    b: i32::MAX,
                    c: DA(CC::C(i128::MAX)),
                },
                x: 255,
            }
            .byte_len(),
            24
        );
        assert_eq!(DE::Z(true, -48).byte_len(), 3);

        #[derive(ByteRepr)]
        enum DF {
            X(bool, BA, u16),
            Y { fixed: DB, x: u8 },
            Z(CA, i16, BA),
        }
        assert_eq!(DF::MIN_BYTE_LEN, 5);
        assert_eq!(DF::MAX_BYTE_LEN, 25);
        assert_eq!(DF::X(false, BA { n: 0 }, 0).byte_len(), 5);
        assert_eq!(DF::X(true, BA { n: 1 }, u16::MAX).byte_len(), 5);
        assert_eq!(
            DF::Y {
                fixed: DB {
                    a: false,
                    b: 17,
                    c: DA(CC::B(true, 5)),
                },
                x: 9,
            }
            .byte_len(),
            11
        );
        assert_eq!(
            DF::Y {
                fixed: DB {
                    a: true,
                    b: i32::MIN,
                    c: DA(CC::C(i128::MIN)),
                },
                x: 255,
            }
            .byte_len(),
            24
        );
        assert_eq!(
            DF::Z(
                CA {
                    named: 3.14,
                    unnamed: true,
                    b: 123456789,
                },
                -12345,
                BA { n: 77 },
            )
            .byte_len(),
            25
        );
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

    #[test]
    fn array() {
        #[derive(ByteRepr, PartialEq, Debug)]
        struct A {
            list: [f32; 3],
        }
        #[derive(ByteRepr, PartialEq, Debug, Clone, Copy)]
        struct B(usize);
        #[derive(ByteRepr, PartialEq, Debug)]
        struct C {
            a: [i8; 9],
            b: [B; 240],
        }

        crate::test_bitrepr_roundtrip!(
            a,
            A,
            A {
                list: [1397.201, -0.0, -4010401.32914]
            }
        );
        crate::test_bitrepr_roundtrip!(
            c,
            C,
            C {
                a: [-127, 0, 84, -5, 39, 100, -128, 127, -99],
                b: [B(usize::MAX); 240]
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
