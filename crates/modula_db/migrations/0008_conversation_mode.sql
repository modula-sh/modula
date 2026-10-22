-- How the conversation renders, fixed at create: `chat` (parsed transcript)
-- or `terminal` (the provider's own TUI in a PTY).
ALTER TABLE conversations ADD COLUMN mode TEXT NOT NULL DEFAULT 'chat';
