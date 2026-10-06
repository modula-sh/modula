-- Where a reply sits in its provider's session: the session it was produced
-- in and the provider's own id for the turn. A chat cloned at that reply forks
-- the session there, so it starts with exactly the context up to that point.
ALTER TABLE conversation_messages ADD COLUMN anchor_session TEXT;
ALTER TABLE conversation_messages ADD COLUMN anchor TEXT;

-- The session and point a cloned conversation's first turn forks from.
ALTER TABLE conversations ADD COLUMN fork_session TEXT;
ALTER TABLE conversations ADD COLUMN fork_anchor TEXT;
