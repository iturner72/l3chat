DROP FUNCTION IF EXISTS match_project_chunks(vector(1536), UUID, float, int);

DROP INDEX IF EXISTS idx_chunk_embeddings_hnsq;
DROP INDEX IF EXISTS idx_threads_project_id;
DROP INDEX IF EXISTS idx_document_chunks_index;
DROP INDEX IF EXISTS idx_document_chunks_document_id;
DROP INDEX IF EXISTS idx_project_documents_project_id;
DROP INDEX IF EXISTS idx_projects_user_id;

ALTER TABLE threads DROP COLUMN IF EXISTS project_id;

DROP TABLE IF EXISTS chunk_embeddings;
DROP TABLE IF EXISTS document_chunks;
DROP TABLE IF EXISTS project_documents;
DROP TABLE IF EXISTS projects;
