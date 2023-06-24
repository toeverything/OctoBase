# How to develop OctoBase server

Welcome to OctoBase, a local-first, yet collaborative database! This document will guide you through the steps of building and running OctoBase.

## Start Server

### Installing Rust Toolchain

Before building OctoBase, you'll need to install the Rust toolchain if you haven't already. You can do this by running the following command in your terminal:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Cloning the Repository

Once you have the Rust toolchain installed, you can clone the OctoBase repository by running the following command:

```sh
git clone https://github.com/toeverything/OctoBase/
```

### Run Keck Server

Use the following command to compile and run the `Keck` server using Cargo:

```sh
cargo run --bin keck
```

The `Keck` server will then start on the 3000 port.

### Run AFFiNE Cloud Server

Use the following command to compile and run the `AFFiNE Cloud` server using Cargo:

```sh
cargo run --bin affine-cloud
```

Note that running the `AFFiNE Cloud` server has some special requirements.

#### Specify Database Address

`AFFiNE Cloud` supports SQLite or PostgreSQL as data backends. If no database address is specified, the server defaults to using an in-memory database. This means that all data will be lost when the server shuts down. Therefore, it is recommended that you specify a database file path for persistence.

If you want to use a SQLite database, set the `DATABASE_URL` environment variable to the database file path, for example:

```sh
export DATABASE_URL=sqlite:///path/to/database.db
```

If you want to use a PostgreSQL database, set the following environment variables to your database connection information:

```sh
export DATABASE_URL=postgres://username:password@hostname/database_name
```

#### Specify Token Key

In `AFFiNE Cloud`, you need to use tokens for authentication and authorization operations. If no token key is specified, the server will use a random key. This means that all users will need to log in again when the server is restarted.

To avoid this situation, set the `SIGN_KEY` environment variable to a persistent and secure key.

### Using Firebase Login

By default, `AFFiNE Cloud` only supports official login provider in AFFiNE. If you want to allow users to log in using your Firebase project, set the `FIREBASE_PROJECT_ID` environment variable to your Firebase project ID.

### Note

If you encounter any issues, welcome to contact the developers for assistance.

## Api Documentation

### Use the api doc in web

1. Set the `JWST_DEV` environment variable.

    ```sh
    export JWST_DEV=1
    ```

2. Start the cloud or keck server.

3. Access http://localhost:3000/api/docs/ to use the Swagger GUI to debug the API.

### Use the api doc in postman

![postman](./assets/building_guide_1.jpg)

For cloud:

-   Use `/api/affine-cloud.json` to import the cloud API into Postman.

For keck:

-   Use `/api/jwst.json` to import the keck API into Postman.
