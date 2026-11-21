CREATE TABLE host_registrations(
 id TEXT PRIMARY KEY,
 name TEXT NOT NULL,
 name_key TEXT NOT NULL,
 revision INTEGER NOT NULL CHECK(revision=1),
 created_sequence INTEGER NOT NULL UNIQUE REFERENCES events(sequence),
 document TEXT NOT NULL
);
CREATE INDEX host_registrations_name_id ON host_registrations(name_key,id);
