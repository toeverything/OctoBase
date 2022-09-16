# Tour of the API

See how JWST API fit together and learn best practices for combining them.

## Get Workspace

```sh
curl 'http://localhost:3000/api/block/AFFiNE'
```

## Get Block

```sh
curl 'http://localhost:3000/api/block/AFFiNE/affine_P2hA3XxAacZ8SKR'
```

## Set Block

```sh
curl 'http://localhost:3000/api/block/AFFiNE/affine_P2hA3XxAacZ8SKR' \
  --data-raw '{"test": 1}'
```

## To be continue...

At this time, you can view the detailed api introduction from our [Swagger Page](/swagger-ui/), we will improve the content here as soon as possible
