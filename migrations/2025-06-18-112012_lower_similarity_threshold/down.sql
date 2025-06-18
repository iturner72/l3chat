-- Revert to the original higher similarity threshold
-- Restore threshold: 0.72 similarity (0.28 cosine distance)

CREATE OR REPLACE FUNCTION match_project_chunks(
    query_embedding VECTOR(1536),
    project_uuid UUID,
    match_threshold float DEFAULT 0.72,
    match_count int DEFAULT 5
)
RETURNS TABLE (
    chunk_id UUID,
    chunk_text TEXT,
    similarity float,
    document_id UUID,
    filename VARCHAR(255),
    chunk_index INTEGER
)
LANGUAGE plpgsql
AS $$
BEGIN
    RETURN QUERY
    SELECT
        dc.id as chunk_id,
        dc.chunk_text,
        1 - (ce.embedding <=> query_embedding) as similarity,
        dc.document_id,
        pd.filename,
        dc.chunk_index
    FROM document_chunks dc
    JOIN chunk_embeddings ce ON dc.id = ce.chunk_id
    JOIN project_documents pd ON dc.document_id = pd.id
    WHERE pd.project_id = project_uuid
        AND 1 - (ce.embedding <=> query_embedding) > match_threshold
    ORDER BY ce.embedding <=> query_embedding
    LIMIT match_count;
END;
$$;
