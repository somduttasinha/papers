-- Your SQL goes here
CREATE TABLE documents (
  id VARCHAR PRIMARY KEY,
  title VARCHAR NOT NULL,
  body TEXT NOT NULL,
  thumbnail_url VARCHAR NOT NULL
)
