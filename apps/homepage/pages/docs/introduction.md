## OctoBase - local-first, yet collaborative database

OctoBase is an offline-available, scalable, self-contained collaborative database implemented based on CRDTs.
It is the core to resolve conflicts between the duplication of data and manage the databases so that real-time collaboration and local-first storage is possible.
It supports local storage and serve-side storage.

### Features

#### Everything is block

OctoBase constructs a basic data structure abstraction, which we call Block.
A block contains: type, creation/modification time, creator, children and content.
Based on the above schema, we can build a block tree and express richer data associations based on them, such as bidirectional links, synchronous references, etc.

#### Conflict-free synchronization

In OctoBase, all Blocks under the same workspace are assigned a unique identifier and are stored in the same Map, which means that Blocks can be merged without conflict.
Blocks also build associations based on children, which allows Blocks to express more complex diagrams.
In addition, we implemented the conflict-free merge of Block itself based on yjs, which makes all CRUDs to Block are CRDT.

#### Unaware synchronization

All blocks are CRDTs, so you can make complex modifications to a block offline, and when you connect to other clients again, the data you generate can be merged with other clients without conflict.
This means that for upper-layer applications, they do not need to care whether the program is already connected to the Internet, so the data synchronization of OctoBase is imperceptible to the upper layer, which allows the upper-layer application to focus on building data models.

#### Multiple platform support

OctoBase is written by Rust and without complex third-party dependencies, it can be easily compiled to almost all common instruction set platforms and operating systems .

OctoBase also provides ffi binding support for common languages. You can use different languages to call OctoBase to build interoperable data.

#### Point-to-point / central server synchronization

OctoBase supports centralized synchronization: use OctoBase to build the server and use OctoBase to synchronize with the server on the client.

And OctoBase also supports point-to-point synchronization: any transmission protocol that supports upstream and downstream can be connected to OctoBase, such as libp2p, webrtc, etc.
