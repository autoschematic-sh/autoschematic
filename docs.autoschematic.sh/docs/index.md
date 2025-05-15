# About Autoschematic

Autoschematic is a new platform for building declarative Ops systems.

Autoschematic fundamentally differs from existing systems, such as Terraform, Cloudformation, and Pulumi,
in the core design of its state model. The state model adopted by Autoschematic tightly integrates with Git,
using pull-requests as the fundamental unit of work, and taking on addressing and transactional models that
map directly to files and commits within a repository.

Aesthetically, the design of Autoschematic can be said to resemble the [filesystem server](https://9p.io/sys/man/4/INDEX.html) concept from Plan 9,
in which resources are addressed by hierarchical paths within a mountpoint.
