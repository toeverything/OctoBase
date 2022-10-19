# How to sync you own data

## Design you data model with `Block`

In JWST, Block will storage in a flat Map like structure, and every block will has some common key-value like this:

```js
Map {
    "block_id1": Block {
        // difference flavor means different block type
        flavor: "affine:test",
        created: 1666158236651,
        updated: 1666158236651,
        children: ["block_id2"],
    }
}
```
