(CRDT = Conflict-free Replicated Data Type)

Hi!

One application of this code would be to make an application like Google Docs, allowing simultaneous edits across different devices, and viewing the full edit history. It is a jumping-off point for the user to make their own CRDTs. I would like to revisit this in the future! But right now my intention is that if you want to use it, just clone it and modify it to suit your needs. 

The model for collaborative editing used by `replicant` is that every user creates an append-only log. Any change tha user makes to the data is appended to their log. By collecting all the logs from all the users, you can redo all their changes and reconstruct the latest version. This means that the size of the CRDT grows with every change made (although this must be the case for anything that stores the full edit history like `replicant` does).

This repo doesn't include any code for syncing over a network. I'll demonstrate how it works with a very simple example - a program to collaboratively change a number. 