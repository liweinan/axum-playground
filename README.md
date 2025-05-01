## Introduction

This project shows how to use Axum with Diesel:

- [GitHub - tokio-rs/axum: Ergonomic and modular web framework built with Tokio, Tower, and Hyper](https://github.com/tokio-rs/axum)
- [GitHub - diesel-rs/diesel: A safe, extensible ORM and Query Builder for Rust](https://github.com/diesel-rs/diesel)

## Features

- PostgreSQL database integration with Diesel ORM
- User management with CRUD operations
- Pagination support
- SQL query execution
- Integration tests
- Docker support for development
- GitHub Actions CI/CD pipeline

## Usage

### Prerequisites

- Docker and Docker Compose
- Rust toolchain
- PostgreSQL client (optional, for direct database access)
- diesel_cli (for database migrations)

### Setup

1. Create a `.env` file with the following variables:
```bash
DATABASE_URL=postgres://foo_user:foo_password@localhost:5432/foo_db
POOL_SIZE=5
```

2. Install diesel_cli:
```bash
cargo install diesel_cli --no-default-features --features postgres
```

3. Start the database:
```bash
docker-compose up -d
```

4. Run migrations:
```bash
diesel migration run
```

### Running the Application

```bash
cargo run
```

### Running Tests

```bash
./test.sh
```

The test script will:
1. Start the PostgreSQL 15 database container
2. Run migrations
3. Start the application
4. Execute integration tests
5. Clean up resources

### API Endpoints

#### Create User
```bash
curl --location --request POST 'http://localhost:3000/users' \
--header 'Content-Type: application/json' \
--data-raw '{
    "username": "test_user"
}'
```

#### Get Users with Pagination
```bash
curl 'http://localhost:3000/users?page=1&page_size=10'
```

#### Find User by ID
```bash
curl 'http://localhost:3000/find_user_by_id/{id}'
```

#### Get All SQL Users
```bash
curl 'http://localhost:3000/find_all_sql_users'
```

### Database Schema

The project uses PostgreSQL 15 with the following schema:

```sql
CREATE TABLE users (
    id VARCHAR PRIMARY KEY,
    username VARCHAR UNIQUE NOT NULL,
    created_at TIMESTAMP,
    meta JSONB
);
```

### Testing

The project includes integration tests that verify:
- User creation with unique usernames
- User listing with pagination
- SQL query functionality
- Error handling and response formats

Tests are located in `tests/integration_test.rs` and can be run using the `test.sh` script.

### Development

The project uses:
- Axum 0.8.4 for the web framework
- Diesel 2.2.10 for database operations
- PostgreSQL 15 for the database
- Docker and Docker Compose for containerization

### CI/CD

The project includes a GitHub Actions workflow that:
1. Sets up the Rust toolchain
2. Installs diesel_cli
3. Installs Docker and Docker Compose
4. Runs the test script
5. Verifies database migrations and table creation