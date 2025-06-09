ALTER TABLE threads ADD COLUMN parent_thread_id VARCHAR(255) REFERENCES threads(id);
ALTER TABLE threads ADD COLUMN branch_point_message_id INTEGER REFERENCES messages(id);
ALTER TABLE threads ADD COLUMN branch_name VARCHAR(255);

