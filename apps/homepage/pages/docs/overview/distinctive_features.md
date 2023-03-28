# Distinctive Features Of OctoBase

This page highlights some of the characteristics of OctoBase that are unusual and which make OctoBase different from many other database.

OctoBase provides several core concepts that are abstractions of the underlying data operations. With the help of these abstractions, developers can read and modify data in a customary way and can synchronize with remote data without conflicts.

### Block

The basic data unit of OctoBase is `Block`, which contains several basic properties, such as flavour, creation time, children and so on. Users can store custom data in `Block` through the API interface.

Flavour is the type of `Block`, which is derived from [particle physics](<https://en.wikipedia.org/wiki/Flavour_(particle_physics)>), which means that all blocks have the same basic properties, when the flavour of `Block` changing, his basic attributes will not change, but the interpretation of user-defined attributes will be different.

Created records the creation time of the `Block`, which is a timestamp in milliseconds.

Children is an array that records the child blocks of the current block. We can build a series of blocks into a top-down tree based on children, or put a parent block into it to form a [directed graph](https://en.wikipedia.org/wiki/Directed_graph).

OctoBase provides a series of interfaces to help you create blocks, organize children of blocks, read and write properties of blocks, and all write operations can be merged with the same source remote end without conflict.

The data structure of the simplest `Block` usually looks like this when it is actually stored:

```js
const Block = {
	'sys:flavour': 'affine:text',
	'sys:created': 1666158236651,
	'sys:children': [],
	'prop:text': 'This is a normal text',
}
```

### Space

`Space` is a collection of blocks. Blocks within a `Space` can become each other's children, but in actual storage, all blocks are placed flat in a kv container, a block id (key) point to a `Block` data (value).

```js
const Space = {
	id1: {
		'sys:flavour': 'affine:text',
		'sys:created': 1666158236651,
		'sys:children': [],
		'prop:text': 'This is a normal text',
	},
	id2: {
		'sys:flavour': 'affine:text',
		'sys:created': 1666158236651,
		'sys:children': ['id1'],
		'prop:text': 'This is a normal text',
	},
	id3: {
		'sys:flavour': 'affine:text',
		'sys:created': 1666158236651,
		'sys:children': ['id1', 'id2'],
		'prop:text': 'This is a normal text',
	},
}
```

### Workspace

`Workspace` is a concept that does not exist as a data entity, we call a collection of `Space` is a `Workspace`.

```js
const Workspace = {
	'space:1': new Space(),
	'space:2': new Space(),
	'space:3': new Space(),
}
```

### Client

When we need to execute an operation(e.g. get block, create block, write props etc.) on a `Workspace`, we need to initialize a `Workspace` instance, which contains all the context information needed to execute any operation in the `Workspace`. This instance is called `Client`

### Client ID

When each `Client` execute the first write operation, it will be assigned a randomly generated unique ID called `Client ID`, which will be included in the underlying historical data, so you can use `Client ID` finds all write operations execute by that `Client`, or differentiates operations performed on a single field by multiple workspace instances.
