-- Add password columns to existing portal_registrations (when table already existed without them).
-- Run this only if portal_registrations exists but has no password_salt/password_hash columns.
-- Apply with:
--   wrangler d1 execute boing-network-db --file=./migrations/2026-03-06-portal-password-alter.sql

ALTER TABLE portal_registrations ADD COLUMN password_salt TEXT;
ALTER TABLE portal_registrations ADD COLUMN password_hash TEXT;
