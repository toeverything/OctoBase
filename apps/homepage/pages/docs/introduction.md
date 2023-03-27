# OctoBase - Local-first, yet collaborative database

OctoBase is an offline-available, scalable, self-contained collaborative database implemented based on CRDTs.
It is the core to resolve conflicts between the duplication of data and manage the databases so that real-time collaboration and local-first storage is possible.
It supports local storage and serve-side storage.

## Everything is block

In traditional relational databases, all related content needs to be stored in the same table. This requirement forces developers to design the overall data architecture before developing, which limits flexibility.

OctoBase introduces a new data structure abstraction called Block. A Block is the basic unit of data, and its basic fields enable it to self-describe its read and write methods and relationships.

By organizing the relationships between different Blocks and storing data in them, developers can achieve more flexibility in adjusting their data architecture. OctoBase adopts memory-based read and write methods, which enhances data access speed.

OctoBase also offers a Block-based full-text indexing and query interface, making it easy for developers to query data through simple calls.

## CRDT-driven P2P synchronization

All Blocks within OctoBase are CRDTs, which are built on the [yrs] and can interact with [blocksuite] / [yjs].

CRDT is a distributed data structure that ensures eventual consistency without requiring a central server or coordination algorithm like Raft. This allows OctoBase to provide high-performance, local-first data read/write services.

OctoBase has implemented a CRDT-based peer-to-peer protocol that allows software based on OctoBase to exchange data directly without the need for a centralized server.

Through OctoBase, users can directly synchronize and share various types of data such as text, images, and code between devices.

This means that for upper-layer applications, they do not have to worry about internet connectivity issues. CRDT enables OctoBase to make offline edits to data, and seamlessly syncs with remote endpoints without any conflicts when back online.

## Multiple platform support

OctoBase is a library written in Rust, with no complex third-party dependencies, making it easy to compile to almost all common instruction set platforms and operating systems.

OctoBase also offers ffi binding support for common programming languages. This means that you can use OctoBase to transfer and process data between different programming languages and platforms, making them work together.

[blocksuite]: https://github.com/toeverything/blocksuite
[yjs]: https://github.com/yjs/yjs
[yrs]: https://github.com/y-crdt/y-crdt
