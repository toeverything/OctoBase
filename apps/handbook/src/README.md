## JWST - MultiPlatform CRDT collaborative database

JWST is a set of collaborative backend and database implemented based on CRDTs.
It is the core to resolve conflicts between the duplication of data and manage the databases so that real-time collaboration and local-first storage is possible.
It supports local storage and serve-side storage.

### Core concepts

#### Everything is block

JWST constructs a basic data structure abstraction, which we call Block.
A block contains: type, creation/modification time, creator, children and content.
Based on the above schema, we can build a block tree and express richer data associations based on them, such as bidirectional links, synchronous references, etc.

#### Conflict-free synchronization

In JWST, all Blocks under the same workspace are assigned a unique identifier and are stored in the same Map, which means that Blocks can be merged without conflict.
Blocks also build associations based on children, which allows Blocks to express more complex diagrams.
In addition, we implemented the conflict-free merge of Block itself based on yjs, which makes all CRUDs to Block are CRDT.

#### Unaware synchronization

All blocks are CRDTs, so you can make complex modifications to a block offline, and when you connect to other clients again, the data you generate can be merged with other clients without conflict.
This means that for upper-layer applications, they do not need to care whether the program is already connected to the Internet, so the data synchronization of JWST is imperceptible to the upper layer, which allows the upper-layer application to focus on building data models.
