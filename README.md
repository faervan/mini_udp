# mini_udp
A minimal, fully synchronous implementation of a reliability protocol on top of UDP.

This was inspired by Glenn Fiedler, who wrote an amazing set of articles about this:
<https://gafferongames.com/categories/building-a-game-network-protocol/>

```rust
use mini_udp::prelude::*;

#[derive(ByteRepr, Debug, PartialEq)]
enum MessageToServer {
    Hello,
    Position([f32; 3]),
}
#[derive(ByteRepr, Debug)]
enum MessageToClient {
    WhatIsYourPosition,
    Bye,
}

const POSITION: [f32; 3] = [-1., 0.004, 2482.3];

let mut server = MultiUdpCommunicator::bind("0.0.0.0:7001");
let mut client = UdpCommunicator::default().connect("0.0.0.0:7001").unwrap();

client.write_ordered(MessageToServer::Hello);
let mut messages_read = 0;
loop {
    client.send().unwrap();
    server.recv(|mut com: UdpCommunicatorMut<_, _>| {
        if let Some(msg) = com.read_ordered() {
            messages_read += 1;
            match msg {
                MessageToServer::Hello =>
                    com.write_ordered(MessageToClient::WhatIsYourPosition),
                MessageToServer::Position(pos) => {
                    assert_eq!(pos, POSITION);
                    com.write_ordered(MessageToClient::Bye);
                }
            }
        }
    });
    server.send();
    client.recv();
    if let Some(msg) = client.read_ordered() {
        messages_read += 1;
        match msg {
            MessageToClient::WhatIsYourPosition => {
                client.write_ordered(MessageToServer::Position(POSITION));
            }
            MessageToClient::Bye => break,
        }
    }
}
assert_eq!(messages_read, 4);
```

## License
All code in this repository is dual-licensed under either

* MIT License ([LICENSE-MIT](LICENSE-MIT) or [http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))
* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or [http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0))

at your option.

### Contributions

Unless you explicitly state otherwise,
any contribution intentionally submitted for inclusion in the work by you,
as defined in the Apache-2.0 license,
shall be dual licensed as above,
without any additional terms or conditions.
