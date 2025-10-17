## GPX file ingester

Instead of strava ingest the gpx data to an encrypted cloud

Pay for encrypted cloud storage, not software features

encrypted clouds are trustless, and compatible with arbitary applications as databases

grant proposal for zk + https://willowprotocol.org/

trustless storage, zk, acl, database interface

### Frontend

the key is having an interface that shows a map of the route, speed, heart rate, power

need comments and sharing... via bluesky?

in the future build a comment system into the users encrypted cloud, so they own their own data

### Journaling

Journaled system for replay?

Depends how much the journal can be compressed.

Need a zk compatible compression algo.

Journaling logic is internal to the orm, not the kv. The required logic can be arbitrarily complex, like diff compression keyed to transaction hashes, because it's internal to the db/orm.

Probably going to keep the shitty one for now.
