## Introduction

This project shows how to use Axum with Diesel:

- [GitHub - tokio-rs/axum: Ergonomic and modular web framework built with Tokio, Tower, and Hyper](https://github.com/tokio-rs/axum)
- [GitHub - diesel-rs/diesel: A safe, extensible ORM and Query Builder for Rust](https://github.com/diesel-rs/diesel)

## Usage

### Create Database

```bash
postgres=# create user foo_user;
CREATE ROLE
postgres=# create database foo_db;
CREATE DATABASE
postgres=# alter database foo_db owner to foo_user;
ALTER DATABASE
postgres=# alter role foo_user with login;
ALTER ROLE
```

### Run Migration

```bash
➤ diesel migration run                                                                                                                                                                                                                                                       11:00:30
Running migration 2022-02-05-134136_add_users_table
```

### Run Service

```bash
$ cargo run
...
Finished dev [unoptimized + debuginfo] target(s) in 3.06s
Running `target/debug/axum-playground`
```

### Test Request


#### Create user


Command: 

```bash
curl --location --request POST 'http://localhost:3000/users' \
--header 'Content-Type: application/json' \
--data-raw '{
    "username":"liweinan1"
}'
```

Create Typed User:

```bash
curl --location --request POST 'http://localhost:3000/typed_users' \
--header 'Content-Type: application/json' \
--data-raw '{
    "username":"liweinan42"
}'
```

Result:

```json
{"id":"66fd9d99-1b3f-4be4-b805-161775caafe0","username":"liweinan"}
```

### Delete User

```bash
curl http://localhost:3000/delete_user_by_id/b61896bc-a449-402e-ba9c-273f0eea052d                                                                                                                                                                                                                                            01:55:44
{"id":"b61896bc-a449-402e-ba9c-273f0eea052d","username":"liweinan999","created_at":"2022-08-17T01:55:44.700301","meta":{"meta":null,"data":{"foo":"1","bar":"1"}}}
```

#### Get user

Run:

```bash
$ curl 'http://localhost:3000/users?page=1&page_size=1'
```

Result:

```json
{"r":true,"d":[[{"id":"66fd9d99-1b3f-4be4-b805-161775caafe0","username":"liweinan"}],2],"e":null}
```

