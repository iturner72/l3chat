CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    external_id VARCHAR NOT NULL,
    provider VARCHAR NOT NULL,
    email VARCHAR,
    username VARCHAR,
    display_name VARCHAR,
    avatar_url VARCHAR,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    
    UNIQUE(external_id, provider)
);

ALTER TABLE threads ADD COLUMN user_id INTEGER REFERENCES users(id);

ALTER TABLE messages ADD COLUMN user_id INTEGER REFERENCES users(id);
