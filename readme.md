(CRDT = Conflict-free Replicated Data Type)

Hi!

This is something I made to learn about Rust, CRDTs, and eventual consistency. One application of this code would be to make an application like Google Docs, allowing simultaneous edits across different devices, and viewing the full edit history. It is a jumping-off point for the user to make their own CRDTs. I would like to revisit this in the future! 

The model for collaborative editing used by `replicant` is that every user creates an append-only log. Any change tha user makes to the data is appended to their log. By collecting all the logs from all the users, you can redo all their changes and reconstruct the latest version. This means that the size of the CRDT grows with every change made (although this must be the case for anything that stores the full edit history like `replicant` does). I haven't tested it but I suspect replicant files would compress very well.

This repo doesn't include any code for syncing over a network. Replicant is completely network-agnostic, so that wouldn't really make sense. What I have implemented is a way of writing replicant files to disk, so they could be synced over dropbox or git. (such a syncing operation will __never__ create merge or syncing conflicts in whatever syncing tool you use).

# Demo

<https://gfycat.com/tartoccasionalchupacabra>
![demo](https://thumbs.gfycat.com/TartOccasionalChupacabra-mobile.mp4)
