//! # Overview
//! A minimal, fully synchronous implementation of a reliability protocol on top of UDP.
//!
//! This was inspired by Glenn Fiedler, who wrote an amazing set of articles about this:
//! <https://gafferongames.com/categories/building-a-game-network-protocol/>
//!
//! The main entry points of this crate are [`UdpCommunicator`](communicator::UdpCommunicator)
//! and [`MultiUdpCommunicator`](communicator::MultiUdpCommunicator), which both wrap
//! [`std::net::UdpSocket`].
//!
//! All messages are (de)serialized by the in-house [`ByteRepr`] trait, which has a derive macro as well:
//! [`ByteRepr`](macro@prelude::ByteRepr).
//!
//! ## Features
//! - [x] Derive byte representations for Enums and Structs.
//! - [x] Send unreliable, reliable, or reliable ordered messages over UDP.
//! - [x] Messages get combined into packets, with a maximum packet size of 1024 bytes
//!   ([`MAX_PACKET_DATA_LEN`](packet::MAX_PACKET_DATA_LEN)).
//! - [x] Handle 1-X communication via a single, shared UDP socket
//!   ([`MultiUdpCommunicator`](communicator::MultiUdpCommunicator)).
//! - [x] Have a fully synchronous, non-blocking API, ideal for game networking.
//! - [ ] Messages cannot be fragmented yet, so **it is not possible to send messages larger than
//!   1024 bytes! (yet)**
//! - [ ] The [`ByteRepr`](macro@prelude::ByteRepr) derive is not very mindful of bandwidth yet
//!   (booleans are padded to 1 byte, strings and vecs use 4 bytes to send their length as u32).
//!
//! ## Example
//! <details>
//! <summary><i>Show example</i></summary>
//!
//! ```rust
//! use mini_udp::prelude::*;
//!
//! #[derive(ByteRepr, Debug, PartialEq)]
//! enum MessageToServer {
//!     Hello,
//!     Position([f32; 3]),
//! }
//! #[derive(ByteRepr, Debug)]
//! enum MessageToClient {
//!     WhatIsYourPosition,
//!     Bye,
//! }
//!
//! const POSITION: [f32; 3] = [-1., 0.004, 2482.3];
//!
//! let mut server = MultiUdpCommunicator::bind("0.0.0.0:7001");
//! // `UdpCommunicator::default()` binds the communicator to "0.0.0.0:0", which lets the OS decide
//! // which port to use.
//! let mut client = UdpCommunicator::default().connect("0.0.0.0:7001").unwrap();
//!
//! // The `write*` methods only add the message to a queue, they won't be send until you explicitly
//! // call `send()`.
//! client.write_ordered(MessageToServer::Hello);
//!
//! let mut messages_read = 0;
//! loop {
//!     // Send all queued messages. This is also responsible for resending reliable packets if
//!     // they have not received an acknowledgement yet.
//!     client.send().unwrap();
//!     // Receive all new packets. You can provide a callback function that will be called for each
//!     // received packet, with a mutable reference to the associated connection.
//!     server.recv(|mut com: UdpCommunicatorMut<_, _>| {
//!         if let Some(msg) = com.read_ordered() {
//!             messages_read += 1;
//!             match msg {
//!                 MessageToServer::Hello =>
//!                     com.write_ordered(MessageToClient::WhatIsYourPosition),
//!                 MessageToServer::Position(pos) => {
//!                     assert_eq!(pos, POSITION);
//!                     com.write_ordered(MessageToClient::Bye);
//!                 }
//!             }
//!         }
//!     });
//!     server.send();
//!     client.recv();
//!     // If we would call `client.read()` here, we would not get any messages because ordered and
//!     // non-ordered receive queues are separated.
//!     if let Some(msg) = client.read_ordered() {
//!         messages_read += 1;
//!         match msg {
//!             MessageToClient::WhatIsYourPosition => {
//!                 client.write_ordered(MessageToServer::Position(POSITION));
//!             }
//!             MessageToClient::Bye => break,
//!         }
//!     }
//! }
//! assert_eq!(messages_read, 4);
//! ```
//! </details>

extern crate self as mini_udp;

mod byte_repr;
pub use byte_repr::{ByteRepr, ByteReprError, StaticByteRepr};

pub mod communicator;
pub mod packet;
mod packet_ack;
pub mod prelude;
/// The ring buffer implementation used to cache reliably send and received packets.
pub mod ring_buffer;

#[doc(hidden)]
pub use tracing;

#[cfg(test)]
mod test {
    use std::borrow::Cow;

    use crate::prelude::*;

    #[test]
    fn min_max_length_derive() {
        #[derive(ByteRepr, Debug)]
        struct AA {}
        assert_eq!(AA::MIN_BYTE_LEN, 0);
        assert_eq!(AA::MAX_BYTE_LEN, 0);
        assert_eq!(AA {}.byte_len(), 0);

        #[derive(ByteRepr, Debug)]
        struct AB();
        assert_eq!(AB::MIN_BYTE_LEN, 0);
        assert_eq!(AB::MAX_BYTE_LEN, 0);
        assert_eq!(AB().byte_len(), 0);

        #[derive(ByteRepr, Debug)]
        struct AC;
        assert_eq!(AC::MIN_BYTE_LEN, 0);
        assert_eq!(AC::MAX_BYTE_LEN, 0);
        assert_eq!(AC.byte_len(), 0);

        #[derive(ByteRepr, Debug)]
        enum AD {
            A,
        }
        assert_eq!(AD::MIN_BYTE_LEN, 0);
        assert_eq!(AD::MAX_BYTE_LEN, 0);
        assert_eq!(AD::A.byte_len(), 0);

        #[derive(ByteRepr, Debug)]
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

        #[derive(ByteRepr, Debug)]
        struct BB(u8);
        assert_eq!(BB::MIN_BYTE_LEN, 1);
        assert_eq!(BB::MAX_BYTE_LEN, 1);
        assert_eq!(BB(0).byte_len(), 1);
        assert_eq!(BB(1).byte_len(), 1);
        assert_eq!(BB(42).byte_len(), 1);
        assert_eq!(BB(128).byte_len(), 1);
        assert_eq!(BB(255).byte_len(), 1);

        #[derive(ByteRepr, Debug)]
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

        #[derive(ByteRepr, Debug)]
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

        #[derive(ByteRepr, Debug)]
        struct CB(i64, i8);
        assert_eq!(CB::MIN_BYTE_LEN, 9);
        assert_eq!(CB::MAX_BYTE_LEN, 9);
        assert_eq!(CB(0, 0).byte_len(), 9);
        assert_eq!(CB(1, 1).byte_len(), 9);
        assert_eq!(CB(-1, -1).byte_len(), 9);
        assert_eq!(CB(i64::MIN, i8::MIN).byte_len(), 9);
        assert_eq!(CB(i64::MAX, i8::MAX).byte_len(), 9);

        #[derive(ByteRepr, Debug)]
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

        #[derive(ByteRepr, Debug)]
        struct DA(CC);
        assert_eq!(DA::MIN_BYTE_LEN, 4);
        assert_eq!(DA::MAX_BYTE_LEN, 17);
        assert_eq!(DA(CC::A { x: 7 }).byte_len(), 5);
        assert_eq!(DA(CC::B(false, 99)).byte_len(), 4);
        assert_eq!(DA(CC::B(true, 1234)).byte_len(), 4);
        assert_eq!(DA(CC::C(0)).byte_len(), 17);
        assert_eq!(DA(CC::C(i128::MAX)).byte_len(), 17);

        #[derive(ByteRepr, Debug)]
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

        #[derive(ByteRepr, Debug)]
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

        #[derive(ByteRepr, Debug)]
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

        #[derive(ByteRepr, Debug)]
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

        #[derive(ByteRepr, Debug)]
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
                    named: 3.149,
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
    #[allow(clippy::enum_variant_names)]
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
                float: 1_031.42
            }
        );
        crate::test_bitrepr_roundtrip!(c, C, C::E(f64::MAX, 4, -193042.04, f64::MIN));
    }

    #[test]
    fn delegated() {
        #[derive(ByteRepr, PartialEq, Debug)]
        struct A {
            x: f64,
            y: B,
        }
        #[derive(ByteRepr, PartialEq, Debug)]
        struct B(u128);

        crate::test_bitrepr_roundtrip!(
            a,
            A,
            A {
                x: -2_409_406.335_3,
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
            list: [f64; 3],
        }
        #[derive(ByteRepr, PartialEq, Debug, Default, Clone, Copy)]
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
                list: [1397.201, -0.0, -4_010_401.329_14]
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

    #[test]
    fn string() {
        #[derive(ByteRepr, PartialEq, Debug)]
        struct A(String);

        crate::test_bitrepr_roundtrip!(a, A, A(String::from("Hello world!")));
    }

    #[test]
    fn cow() {
        #[derive(ByteRepr, PartialEq, Debug)]
        #[allow(clippy::owned_cow)]
        struct A<'a>(Cow<'a, String>);

        #[derive(ByteRepr, PartialEq, Debug)]
        struct B<'a>(Cow<'a, str>);

        let s = String::from("What a beautiful day");
        crate::test_bitrepr_roundtrip!(a, A, A(Cow::Borrowed(&s)));

        let s2 = "@€~ÜÖÄ²µå";
        crate::test_bitrepr_roundtrip!(b, B, B(Cow::Borrowed(s2)));

        #[derive(ByteRepr, PartialEq, Debug)]
        struct C<'a>(Cow<'a, u128>);

        assert_eq!(C::BYTE_LEN, 16);
        crate::test_bitrepr_roundtrip!(c, C, C(Cow::Borrowed(&u128::MAX)));

        #[derive(ByteRepr, PartialEq, Debug)]
        struct D<'a> {
            msg: Cow<'a, str>,
            n: String,
        }
        crate::test_bitrepr_roundtrip!(
            d,
            D,
            D {
                msg: "What does the cow say?".to_string().into(),
                n: "MOOOOHHHH!".to_string()
            }
        );

        #[derive(ByteRepr, PartialEq, Debug)]
        struct E<'a> {
            a: String,
            b: Cow<'a, str>,
            c: String,
        }
        crate::test_bitrepr_roundtrip!(
            e,
            E,
            E {
                a: "a".to_string(),
                b: "b".to_string().into(),
                c: "c".to_string()
            }
        );
    }

    #[test]
    fn option() {
        crate::test_bitrepr_roundtrip!(o, Option::<String>, Some(String::from("Hello Option!")));
        crate::test_bitrepr_roundtrip!(o, Option::<String>, None);

        #[derive(ByteRepr, Debug, PartialEq)]
        struct A(Option<bool>);
        crate::test_bitrepr_roundtrip!(a, A, A(Some(false)));
        crate::test_bitrepr_roundtrip!(a, A, A(None));

        #[derive(ByteRepr, Debug, PartialEq)]
        #[allow(clippy::owned_cow)]
        struct B<'a> {
            a: String,
            b: Option<String>,
            c: usize,
            d: Option<Cow<'a, String>>,
            e: bool,
            f: String,
        }
        crate::test_bitrepr_roundtrip!(
            b,
            B,
            B {
                a: "Hello world".to_string(),
                b: None,
                c: 299,
                d: Some(Cow::Borrowed(&"Do you like cats?".to_string())),
                e: true,
                f: "I don't know what to write anymore".to_string()
            }
        );
    }

    #[macro_export]
    macro_rules! test_bitrepr_roundtrip {
        ($binding:ident, $ty:ty, $init:expr) => {
            let mut buf = [0; <$ty>::MAX_BYTE_LEN];
            let $binding = $init;
            assert!($binding.write_to_bytes(&mut buf).is_ok());
            assert_eq!(<$ty>::from_bytes(&buf).unwrap(), $binding);
        };
    }
}
