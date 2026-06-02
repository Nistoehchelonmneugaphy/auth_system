CREATE TABLE join_tokens (
    primary_key  BIGSERIAL PRIMARY KEY,
    server_nonce TEXT      NOT NULL,
    session_token TEXT     NOT NULL,
    join_token   TEXT      NOT NULL UNIQUE,
    UNIQUE (server_nonce, session_token)
);
