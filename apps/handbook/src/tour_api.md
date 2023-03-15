# Tour of the API

See how OctoBase API fit together and learn best practices for combining them.

## Workspace

The workspace stores a lot of Blocks, and Blocks under the same Workspace can refer to each other.

#### Get Workspace

> This api returns all content under the workspace

```sh
curl -X 'GET' 'http://localhost:3000/api/block/AFFiNE'
```

#### Create Workspace

> This api create a new workspace, and return the workspace content.

```sh
curl -X 'POST' 'http://localhost:3000/api/block/AFFiNE'
```

#### Delete Workspace

> This API deletes the workspace and all blocks under the workspace.

```sh
curl -X 'DELETE' 'http://localhost:3000/api/block/AFFiNE'
```

## Block

The block is the basic data structure of OctoBase, you can use below apis to manipulate the block itself.

#### Get Block

> This api will return the block content if block exists.

```sh
curl -X 'GET' 'http://localhost:3000/api/block/AFFiNE/affine_P2hA3XxAacZ8SKR'
```

#### Create or set Block

> This api create a new block, and set the block content.
>
> If block exists, it will update the block content.
>
> You can set null value to delete a key in block content.
>
> This api will return the block content if create or set successfully.

````sh

```sh
curl -H "Content-Type: application/json"  'http://localhost:3000/api/block/AFFiNE/affine_P2hA3XxAacZ8SKR' --data-raw '{"test": 1}'
````

#### Delete Block

> This API delete the block if exists.

```sh
curl -X 'DELETE' 'http://localhost:3000/api/block/AFFiNE/affine_P2hA3XxAacZ8SKR'
```

## Block Tree

Block tree is built based on the children field in Block, the following api can be used to modify children.

#### Insert children

> This api insert a new child to the block.
>
> This api receives the following data structure:
>
> ```ts
> type Schema = {
> 	block_id: string // what block you want to insert
> 	after?: string // after which children block do you want to insert
> 	before?: string // before which children block do you want to insert
> 	pos?: number // insert at a specific index
> }
> ```

````

```sh
curl -X 'POST' \
  'http://localhost:3000/api/block/AFFiNE/affine_P2hA3XxAacZ8SKR/insert' \
  -H 'accept: */*' \
  -H 'Content-Type: application/json' \
  -d '{
    "block_id": "affine_Rf4rMzua7EA2CVG",
    "pos": 0
  }'
````

#### Remove children

> This api remove a block children.
>
> This api receives the following data structure:
>
> ```ts
> type Schema = {
> 	block_id: string // what block you want to remove
> }
> ```

```sh
curl -X 'POST' \
  'http://localhost:3000/api/block/AFFiNE/affine_P2hA3XxAacZ8SKR/remove' \
  -H 'accept: */*' \
  -H 'Content-Type: application/json' \
  -d '{
    "block_id": "affine_Rf4rMzua7EA2CVG",
  }'
```

## More detail

At this time, you can view the detailed api introduction from our [Swagger Page](/swagger-ui/), and try request online.
