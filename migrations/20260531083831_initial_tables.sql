CREATE TABLE "users" (
  "primary_key" bigserial NOT NULL,
  "user_id" text NOT NULL,
  "username" text NOT NULL,
  "password_hash" text NOT NULL,
  "subscription_expires_at_timestamp" bigint NOT NULL,
  PRIMARY KEY ("primary_key"),
  UNIQUE ("user_id")
);

CREATE TABLE "session_tokens" (
  "primary_key" bigserial NOT NULL,
  "token" text NOT NULL,
  "user_id" text NOT NULL,
  "expires_at_timestamp" bigint NOT NULL,
  PRIMARY KEY ("primary_key"),
  FOREIGN KEY ("user_id") REFERENCES "users" ("user_id") ON DELETE NO ACTION,
  UNIQUE ("token")
);
