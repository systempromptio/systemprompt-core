# systemprompt-engine-loro

The Loro CRDT engine behind live document sessions. It implements
`systemprompt_files::documents::engine::DocumentEngine` with `loro = "=1.17.1"`,
so `systemprompt-files` itself links no CRDT crate.

**Layer**: App. Enabled through the facade's `files-collab-loro` feature; a
client must speak the same Loro wire version ([`WIRE_VERSION`]).
