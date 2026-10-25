ALTER TABLE legura_posts ADD COLUMN content_format TEXT NOT NULL DEFAULT 'plain-text'
  CHECK (content_format IN ('plain-text', 'legura-richtext'));
ALTER TABLE legura_posts ADD COLUMN content_document TEXT
  CHECK (
    (content_format = 'plain-text' AND content_document IS NULL)
    OR (content_format = 'legura-richtext' AND content_document IS NOT NULL
        AND length(CAST(content_document AS BLOB)) BETWEEN 1 AND 32768)
  );
