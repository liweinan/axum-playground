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

```bash
curl --location --request POST 'http://localhost:3000/users' \
--header 'Content-Type: application/json' \
--data-raw '{
    "username":"liweinan"
}'
{"id":"66fd9d99-1b3f-4be4-b805-161775caafe0","username":"liweinan"}
```
