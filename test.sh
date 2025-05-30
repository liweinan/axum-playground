#!/bin/bash

# Start the database container
echo "Starting database container..."
docker-compose up -d

# Wait for the database to be ready
echo "Waiting for database to be ready..."
for i in {1..300}; do
    if docker-compose exec postgres pg_isready -U foo_user -d foo_db; then
        echo "Database is ready!"
        break
    fi
    if [ $i -eq 300 ]; then
        echo "Database failed to start within 5 minutes"
        docker-compose down
        exit 1
    fi
    sleep 1
done

# Set the database URL with password
export DATABASE_URL="postgres://foo_user:foo_password@localhost:5432/foo_db"

# Run migrations and wait for completion
echo "Running migrations..."
if ! diesel migration run; then
    echo "Migrations failed to run"
    docker-compose down
    exit 1
fi

# Verify tables exist
echo "Verifying database tables..."
if ! docker-compose exec postgres psql -U foo_user -d foo_db -c "\dt" | grep -q "users"; then
    echo "Users table not found after migrations"
    docker-compose down
    exit 1
fi

# Run the application in the background
echo "Starting the application..."
cargo run &
APP_PID=$!

# Wait for the application to start (with timeout)
echo "Waiting for application to start..."
for i in {1..300}; do
    if curl -s http://localhost:3000/ > /dev/null; then
        echo "Application is ready!"
        break
    fi
    if [ $i -eq 300 ]; then
        echo "Application failed to start within 5 minutes"
        docker-compose down
        exit 1
    fi
    sleep 1
done

# Run tests
echo "Running tests..."
cargo test

# Clean up
echo "Cleaning up..."
kill $APP_PID
docker-compose down 